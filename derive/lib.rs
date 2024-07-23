extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, token::Comma, Attribute, Data, DeriveInput, Field, Fields,
    LitStr, Variant, WherePredicate,
};

#[derive(Clone, Debug, Default)]
struct PrettyAttributes {
    // Field: Skip this field - don't include it in the output
    skip: bool,
    // Field: Indent this field - add a newline and indent before and after
    indent: bool,
    // Field: Rename this field - use the given string as the field name
    rename: Option<String>,
    // Field: Use the given function to get the value of this field
    getter: Option<String>,
    // Container: Verbose output - include field names in output
    verbose: bool,
}

fn parse_pretty_attrs(attrs: &[Attribute]) -> PrettyAttributes {
    let mut pretty_attrs = PrettyAttributes::default();

    for attr in attrs {
        if !attr.path().is_ident("pprint") {
            continue;
        }

        // Parse the attribute; we don't care if this fails as we'll silently default
        let _ = attr.parse_nested_meta(|meta| {
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
        });
    }

    pretty_attrs
}

fn apply_pretty_doc_attributes(
    field_doc: &proc_macro2::TokenStream,
    pretty_attr: &PrettyAttributes,
) -> proc_macro2::TokenStream {
    let mut doc = quote! { #field_doc };

    if pretty_attr.indent {
        doc = quote! { (#doc).indent() };
    }

    doc
}

/// Derive the Pretty trait for a struct or enum
///
/// This macro will generate a From implementation for the given struct or enum.
/// The generated From implementation will convert the struct or enum into a pprint::Doc<'a>, where the Doc lifetime
/// is either the lifetime of the struct or enum, or 'a if no lifetime is specified.
/// Example:
/// ```
/// use pprint::Doc;
/// use pprint_derive::Pretty;
/// #[derive(Pretty)]
/// struct Hey {
///     a: u32,
///     b: u32,
/// }
/// let hey = Hey { a: 1, b: 2 };
/// let doc: Doc = hey.into();
/// ```
#[proc_macro_derive(Pretty, attributes(pprint))]
pub fn pprint_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let pretty_attributes = parse_pretty_attrs(&input.attrs);

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // A Doc needs a lifetime - if the user has specified one, use it, otherwise use 'a
    let doc_lifetime = match generics.lifetimes().next() {
        Some(lt) => lt.lifetime.clone(),
        None => parse_quote!('a),
    };

    let doc_match = match &input.data {
        Data::Struct(data_struct) => {
            generate_struct_match(name, &data_struct.fields, &pretty_attributes)
        }
        Data::Enum(data_enum) => generate_enum_match(name, &data_enum.variants, &pretty_attributes),
        _ => panic!("Only structs and enums are supported."),
    };

    // If there's a where clause extant, we want to preserve it, else we want to create a new one
    let mut new_where_clause = where_clause
        .map(|wc| wc.predicates.clone())
        .unwrap_or_else(syn::punctuated::Punctuated::new);

    // Every generic type needs to be constrained to Into<Doc<'a>>
    let new_generic_predicates = generics.type_params().map(|tp| -> WherePredicate {
        let ident = &tp.ident;
        parse_quote! { #ident : Into<Doc<#doc_lifetime>> }
    });
    // Every lifetime needs to be constrained to 'a
    let new_lifetime_predicates = generics.lifetimes().map(|lt| -> WherePredicate {
        let lifetime = &lt.lifetime;
        parse_quote! { #lifetime : 'a }
    });
    new_where_clause.extend(new_generic_predicates);
    new_where_clause.extend(new_lifetime_predicates);

    // Create the From implementation
    let expanded = quote! {
        impl #impl_generics From<#name #ty_generics> for pprint::Doc<#doc_lifetime>
        where
            #new_where_clause
        {
            fn from(_self: #name #ty_generics) -> Self {
                use pprint::{concat, indent, wrap, join, Doc, Join, Wrap, Group, Indent, Dedent};
                #doc_match
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_struct_fields_match(fields: &Fields) -> Vec<proc_macro2::TokenStream> {
    let format_key_value = |field_ident: &Option<syn::Ident>, field: &Field| {
        let pretty_attrs = parse_pretty_attrs(&field.attrs);
        if pretty_attrs.skip {
            return None;
        }
        let field_name = pretty_attrs.rename.clone().unwrap_or_else(|| {
            field_ident
                .as_ref()
                .map(|ident| ident.to_string())
                .unwrap_or_else(|| "".to_string())
        });

        let is_generic_type = matches!(field.ty, syn::Type::Path(_));

        // If the type is a generic type, we need to call into() on it to convert it to a Doc
        let field_doc = if is_generic_type {
            quote! { _self.#field_ident.into() }
        } else {
            quote! { Doc::from(_self.#field_ident) }
        };
        let field_doc = apply_pretty_doc_attributes(&field_doc, &pretty_attrs);
        let field_doc = quote! {
            Doc::Concat(vec![
                Doc::from(#field_name),
                Doc::from(": "),
                #field_doc,
            ])
        };
        // Doc of the form: "key: value"
        Some(field_doc)
    };

    // Generate the match arms for each field
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .filter_map(|field| {
                let field_ident = &field.ident;
                format_key_value(field_ident, field)
            })
            .collect(),

        // If it's unnamed, we need to generate a field name for each field
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                let field_ident = Some(format_ident!("field_{}", i));
                format_key_value(&field_ident, field)
            })
            .collect(),
        Fields::Unit => vec![],
    }
}

fn generate_struct_match(
    ident: &syn::Ident,
    fields: &Fields,
    pretty_attributes: &PrettyAttributes,
) -> proc_macro2::TokenStream {
    let name = pretty_attributes
        .rename
        .clone()
        .unwrap_or_else(|| ident.to_string());

    let fields_match = generate_struct_fields_match(fields);

    // TODO: Fix: hack to remove the unused variable warning when the field is ignored.
    let named_fields = fields.into_iter().filter_map(|field| field.ident.clone());

    match fields {
        Fields::Named(_) | Fields::Unnamed(_) => {
            let body = quote! {
                vec![#(#fields_match,)*]
                        .join(Doc::from(", ") + Doc::Hardline)
                        .group()
                        .wrap("{", Doc::from("}").dedent())
                        .indent()
            };
            let header = quote! {
                Doc::from(format!("{} ", #name)).group().indent()
            };
            let doc_match = if pretty_attributes.verbose {
                quote! {
                    Doc::Concat(vec![#header, #body])
                }
            } else {
                body
            };

            // let doc_match = quote! { Doc::Concat(vec![#(#fields_match,)*]) };

            // let doc_match =  quote! { Doc::Null };
        

            quote! {
                // The hack to remove the unused variable warning when the field is ignored.
                (#((&_self.#named_fields),)*);
                // The actual implementation
                #doc_match
            }
        }
        Fields::Unit => {
            quote! {
                Doc::from(stringify!(#ident))
            }
        }
    }
}

fn generate_variants_match(
    variant: &syn::Variant,
    constructor: &proc_macro2::TokenStream,
    pretty_attributes: &PrettyAttributes,
) -> Option<proc_macro2::TokenStream> {
    let pretty_attrs = parse_pretty_attrs(&variant.attrs);

    if pretty_attrs.skip {
        return None;
    }

    let variant_name = pretty_attrs
        .rename
        .clone()
        .unwrap_or_else(|| variant.ident.to_string());

    let field_bindings = match &variant.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|field| quote! { #field.ident })
            .collect(),

        // If it's unnamed (most variant fields are), we need to generate a field name for each field
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let ident = format_ident!("field_{}", i);
                quote! { #ident }
            })
            .collect(),
        Fields::Unit => {
            vec![quote! {
                #variant_name
            }]
        }
    };

    // If there's only one field, we don't need to wrap it in a tuple
    let field_bindings_tup = if field_bindings.len() == 1 {
        quote! { #(#field_bindings),* }
    } else {
        quote! { (#(#field_bindings),*) }
    };

    // If the variant has a getter, we need to call it to get the value of the field
    let field_doc = match pretty_attrs.getter.clone() {
        Some(getter) => {
            let getter = syn::parse_str::<syn::Expr>(&getter).unwrap();
            quote! {
                #getter(&#field_bindings_tup)
            }
        }
        None => field_bindings_tup,
    };
    let field_doc = quote! {
        Doc::from(#field_doc)
    };
    let field_doc = apply_pretty_doc_attributes(&field_doc, &pretty_attrs);

    // If in verbose mode, we need to wrap the field doc in a tuple,
    // but not if the variant has no fields
    let field_doc = if pretty_attributes.verbose && !matches!(variant.fields, Fields::Unit) {
        quote! {
            Doc::Concat(vec![
                Doc::from(#variant_name),
                Doc::from(#field_doc)
                .wrap("(", ")")
            ])
        }
    } else {
        field_doc
    };

    // Generate the match arms for each field
    let match_arms = match &variant.fields {
        Fields::Named(_) => {
            quote! {
                #constructor { #(#field_bindings),* } => #field_doc
            }
        }
        Fields::Unnamed(_) => {
            quote! {
                #constructor(#(#field_bindings),*) => #field_doc
            }
        }
        Fields::Unit => {
            quote! {
                #constructor =>  #field_doc
            }
        }
    };
    Some(match_arms)
}

fn generate_enum_match(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<Variant, Comma>,
    pretty_attributes: &PrettyAttributes,
) -> proc_macro2::TokenStream {
    let format_variant = |variant: &Variant| {
        let variant_ident = &variant.ident;
        let constructor = quote! { #name::#variant_ident };
        generate_variants_match(variant, &constructor, pretty_attributes)
    };
    let variants_match = variants.into_iter().filter_map(format_variant);

    quote! {
        match _self {
           #(#variants_match,)*
           // Ensure no variant is missing
           _ => Doc::Null
        }
    }
}
