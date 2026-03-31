extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Field, Fields, LitStr, Variant, WherePredicate,
    parse_macro_input, parse_quote, token::Comma,
};

#[derive(Clone, Debug, Default)]
struct PrettyAttributes {
    skip: bool,
    indent: bool,
    rename: Option<String>,
    getter: Option<String>,
    verbose: bool,
}

fn parse_pretty_attrs(attrs: &[Attribute]) -> PrettyAttributes {
    let mut pretty_attrs = PrettyAttributes::default();

    for attr in attrs {
        if !attr.path().is_ident("pprint") {
            continue;
        }

        if let Err(error) = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                pretty_attrs.skip = true;
                Ok(())
            } else if meta.path.is_ident("indent") {
                pretty_attrs.indent = true;
                Ok(())
            } else if meta.path.is_ident("verbose") {
                pretty_attrs.verbose = true;
                Ok(())
            } else if meta.path.is_ident("rename") {
                pretty_attrs.rename = meta.value()?.parse::<LitStr>()?.value().into();
                Ok(())
            } else if meta.path.is_ident("getter") {
                pretty_attrs.getter = meta.value()?.parse::<LitStr>()?.value().into();
                Ok(())
            } else {
                Err(syn::Error::new_spanned(
                    meta.path,
                    "Unknown pprint attribute",
                ))
            }
        }) {
            panic!("Invalid #[pprint(...)] attribute: {error}");
        }
    }

    pretty_attrs
}

/// Derive the Pretty trait for a struct or enum.
///
/// Generates a `Pretty<'a>` implementation that writes formatting ops
/// into an `FmtBuilder`.
///
/// ```ignore
/// use pprint::{Pretty, pprint, PRINTER};
///
/// #[derive(Pretty)]
/// struct Hey { a: u32, b: u32 }
///
/// let hey = Hey { a: 1, b: 2 };
/// println!("{}", pprint(&hey, PRINTER));
/// ```
#[proc_macro_derive(Pretty, attributes(pprint))]
pub fn pprint_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let pretty_attributes = parse_pretty_attrs(&input.attrs);

    let name = &input.ident;
    let generics = &input.generics;
    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Pretty needs a lifetime — use the type's first lifetime or introduce 'a
    let has_lifetime = generics.lifetimes().next().is_some();
    let doc_lifetime: syn::Lifetime = match generics.lifetimes().next() {
        Some(lt) => lt.lifetime.clone(),
        None => parse_quote!('a),
    };

    let body = match &input.data {
        Data::Struct(data_struct) => {
            generate_struct_body(name, &data_struct.fields, &pretty_attributes)
        }
        Data::Enum(data_enum) => generate_enum_body(name, &data_enum.variants, &pretty_attributes),
        _ => panic!("Only structs and enums are supported."),
    };

    // Build impl generics: inject 'a if the type doesn't have a lifetime
    let mut impl_generics_params: syn::punctuated::Punctuated<syn::GenericParam, syn::Token![,]> =
        syn::punctuated::Punctuated::new();
    if !has_lifetime {
        impl_generics_params.push(parse_quote!(#doc_lifetime));
    }
    for param in generics.params.iter() {
        impl_generics_params.push(param.clone());
    }

    // Build where clause: every generic type T: Pretty<'a>, every lifetime: 'a
    let mut new_where_clause = where_clause
        .map(|wc| wc.predicates.clone())
        .unwrap_or_else(syn::punctuated::Punctuated::new);

    let new_generic_predicates = generics.type_params().map(|tp| -> WherePredicate {
        let ident = &tp.ident;
        parse_quote! { #ident : pprint::Pretty<#doc_lifetime> }
    });
    let new_lifetime_predicates = generics.lifetimes().map(|lt| -> WherePredicate {
        let lifetime = &lt.lifetime;
        parse_quote! { #lifetime : #doc_lifetime }
    });
    new_where_clause.extend(new_generic_predicates);
    new_where_clause.extend(new_lifetime_predicates);

    let expanded = quote! {
        impl<#impl_generics_params> pprint::Pretty<#doc_lifetime> for #name #ty_generics
        where
            #new_where_clause
        {
            fn pretty(&#doc_lifetime self, __b: &mut pprint::FmtBuilder<#doc_lifetime>) {
                #body
            }
        }
    };

    TokenStream::from(expanded)
}

// ── Struct codegen ──────────────────────────────────────────────────────────

fn generate_field_emissions(fields: &Fields) -> Vec<proc_macro2::TokenStream> {
    let emit_field =
        |field_name: String, field_access: proc_macro2::TokenStream, field: &Field| {
            let pretty_attrs = parse_pretty_attrs(&field.attrs);
            if pretty_attrs.skip {
                return None;
            }
            let display_name = pretty_attrs.rename.clone().unwrap_or(field_name);

            let mut emit = quote! {
                pprint::Pretty::pretty(&#field_access, __b);
            };

            if pretty_attrs.indent {
                emit = quote! {
                    __b.indent(|__b| {
                        #emit
                    });
                };
            }

            let label = format!("{display_name}: ");
            Some(quote! {
                __b.str(#label);
                #emit
            })
        };

    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .filter_map(|field| {
                let ident = field.ident.as_ref().unwrap();
                let name = ident.to_string();
                let access = quote! { self.#ident };
                emit_field(name, access, field)
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                let name = format!("field_{}", i);
                let idx = syn::Index::from(i);
                let access = quote! { self.#idx };
                emit_field(name, access, field)
            })
            .collect(),
        Fields::Unit => vec![],
    }
}

fn generate_struct_body(
    ident: &syn::Ident,
    fields: &Fields,
    pretty_attributes: &PrettyAttributes,
) -> proc_macro2::TokenStream {
    let name = pretty_attributes
        .rename
        .clone()
        .unwrap_or_else(|| ident.to_string());

    match fields {
        Fields::Named(_) | Fields::Unnamed(_) => {
            let field_emissions = generate_field_emissions(fields);
            let n = field_emissions.len();

            // Build separators between fields
            let fields_with_seps: Vec<_> = field_emissions
                .into_iter()
                .enumerate()
                .map(|(i, emit)| {
                    if i + 1 < n {
                        quote! {
                            #emit
                            __b.comma_sep();
                        }
                    } else {
                        emit
                    }
                })
                .collect();

            let body = quote! {
                __b.wrap(b'{', b'}', |__b| {
                    #(#fields_with_seps)*
                });
            };

            if pretty_attributes.verbose {
                quote! {
                    __b.str(#name);
                    __b.char(b' ');
                    #body
                }
            } else {
                body
            }
        }
        Fields::Unit => {
            quote! {
                __b.str(#name);
            }
        }
    }
}

// ── Enum codegen ────────────────────────────────────────────────────────────

fn generate_variant_arm(
    variant: &Variant,
    constructor: &proc_macro2::TokenStream,
    pretty_attributes: &PrettyAttributes,
) -> Option<proc_macro2::TokenStream> {
    let pretty_attrs = parse_pretty_attrs(&variant.attrs);

    if pretty_attrs.skip {
        let panic_msg = format!(
            "derive(Pretty): variant `{}` is marked #[pprint(skip)] and cannot be formatted",
            variant.ident
        );
        let arm = match &variant.fields {
            Fields::Named(_) => quote! { #constructor { .. } => panic!(#panic_msg) },
            Fields::Unnamed(_) => quote! { #constructor(..) => panic!(#panic_msg) },
            Fields::Unit => quote! { #constructor => panic!(#panic_msg) },
        };
        return Some(arm);
    }

    let variant_name = pretty_attrs
        .rename
        .clone()
        .unwrap_or_else(|| variant.ident.to_string());

    // Generate bindings and the body
    match &variant.fields {
        Fields::Named(fields) => {
            let bindings: Vec<_> = fields.named.iter().map(|f| &f.ident).collect();
            let body = generate_variant_body(&variant_name, &variant.fields, &bindings, &pretty_attrs, pretty_attributes);
            Some(quote! { #constructor { #(#bindings),* } => { #body } })
        }
        Fields::Unnamed(fields) => {
            let bindings: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, _)| format_ident!("field_{}", i))
                .collect();
            let body = generate_variant_body(&variant_name, &variant.fields, &bindings, &pretty_attrs, pretty_attributes);
            Some(quote! { #constructor(#(#bindings),*) => { #body } })
        }
        Fields::Unit => {
            let body = if pretty_attributes.verbose {
                quote! { __b.str(#variant_name); }
            } else {
                quote! { __b.str(#variant_name); }
            };
            Some(quote! { #constructor => { #body } })
        }
    }
}

fn generate_variant_body(
    variant_name: &str,
    _fields: &Fields,
    bindings: &[impl quote::ToTokens],
    variant_attrs: &PrettyAttributes,
    container_attrs: &PrettyAttributes,
) -> proc_macro2::TokenStream {
    // NOTE: enum variant bindings are already references (matched on &self),
    // so we use `#binding` not `&#binding` — the binding IS &'a T already.

    // If only one field and not verbose, emit just the field value
    if bindings.len() == 1 && !container_attrs.verbose {
        let binding = &bindings[0];

        let mut emit = if let Some(getter) = &variant_attrs.getter {
            let getter_expr = syn::parse_str::<syn::Expr>(getter).unwrap_or_else(|e| {
                panic!("derive(Pretty): invalid getter expression `{getter}`: {e}")
            });
            quote! { pprint::Pretty::pretty(&(#getter_expr)(#binding), __b); }
        } else {
            quote! { pprint::Pretty::pretty(#binding, __b); }
        };

        if variant_attrs.indent {
            emit = quote! { __b.indent(|__b| { #emit }); };
        }

        if container_attrs.verbose {
            return quote! {
                __b.str(#variant_name);
                __b.char(b'(');
                #emit
                __b.char(b')');
            };
        }
        return emit;
    }

    // Multiple fields — use getter if present
    if let Some(getter) = &variant_attrs.getter {
        let getter_expr = syn::parse_str::<syn::Expr>(getter).unwrap_or_else(|e| {
            panic!("derive(Pretty): invalid getter expression `{getter}`: {e}")
        });
        let binding_tuple = if bindings.len() == 1 {
            let b = &bindings[0];
            quote! { #b }
        } else {
            quote! { (#(#bindings),*) }
        };
        let mut emit = quote! { pprint::Pretty::pretty(&(#getter_expr)(&#binding_tuple), __b); };
        if variant_attrs.indent {
            emit = quote! { __b.indent(|__b| { #emit }); };
        }
        if container_attrs.verbose {
            return quote! {
                __b.str(#variant_name);
                __b.char(b'(');
                #emit
                __b.char(b')');
            };
        }
        return emit;
    }

    // Multiple fields without getter
    let n = bindings.len();
    let field_emissions: Vec<_> = bindings
        .iter()
        .enumerate()
        .map(|(i, binding)| {
            let emit = quote! { pprint::Pretty::pretty(#binding, __b); };
            if i + 1 < n {
                quote! {
                    #emit
                    __b.comma_sep();
                }
            } else {
                emit
            }
        })
        .collect();

    let body = quote! {
        __b.wrap(b'(', b')', |__b| {
            #(#field_emissions)*
        });
    };

    if container_attrs.verbose {
        quote! {
            __b.str(#variant_name);
            #body
        }
    } else {
        body
    }
}

fn generate_enum_body(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<Variant, Comma>,
    pretty_attributes: &PrettyAttributes,
) -> proc_macro2::TokenStream {
    let arms: Vec<_> = variants
        .iter()
        .filter_map(|variant| {
            let variant_ident = &variant.ident;
            let constructor = quote! { #name::#variant_ident };
            generate_variant_arm(variant, &constructor, pretty_attributes)
        })
        .collect();

    quote! {
        match self {
            #(#arms,)*
        }
    }
}
