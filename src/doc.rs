use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use regex::Regex;

/// A document that can be pretty printed.
/// This is the core type of the library.
/// It is an enum that represents the different ways a document can be printed.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Doc<'a> {
    Null,
    String(Cow<'a, str>),

    Concat(Vec<Doc<'a>>),

    Group(Box<Doc<'a>>),

    Indent(Box<Doc<'a>>),
    Dedent(Box<Doc<'a>>),

    Join(Box<Doc<'a>>, Vec<Doc<'a>>),
    SmartJoin(Box<Doc<'a>>, Vec<Doc<'a>>),

    IfBreak(Box<Doc<'a>>, Box<Doc<'a>>),

    Hardline,
    Softline,
    Mediumline,
    Line,
}

impl<'a> std::ops::Add for Doc<'a> {
    type Output = Doc<'a>;

    fn add(self, other: Doc<'a>) -> Doc<'a> {
        match (self, other) {
            (Doc::Concat(mut docs), other) => {
                docs.push(other);
                Doc::Concat(docs)
            }
            (s, Doc::Concat(mut docs)) => {
                docs.insert(0, s);
                Doc::Concat(docs)
            }
            (s, other) => Doc::Concat(vec![s, other]),
        }
    }
}

/// Group a document if it contains a line break.
/// A group is a document that is printed on a single line if it fits the page,
/// otherwise it is printed with line breaks.
pub fn group<'a>(doc: impl Into<Doc<'a>>) -> Doc<'a> {
    Doc::Group(Box::new(doc.into()))
}

/// Concatenate a vector of documents into a single document.
pub fn concat<'a>(docs: Vec<impl Into<Doc<'a>>>) -> Doc<'a> {
    Doc::Concat(docs.into_iter().map(|d| d.into()).collect())
}

/// Enwrap a document with two other documents, `left` and `right`.
pub fn wrap<'a>(
    left: impl Into<Doc<'a>>,
    doc: impl Into<Doc<'a>>,
    right: impl Into<Doc<'a>>,
) -> Doc<'a> {
    concat(vec![left.into(), doc.into(), right.into()])
}

/// Join a vector of documents on a separator.
pub fn join<'a>(sep: impl Into<Doc<'a>>, docs: Vec<impl Into<Doc<'a>>>) -> Doc<'a> {
    Doc::Join(
        Box::new(sep.into()),
        docs.into_iter().map(|d| d.into()).collect(),
    )
}

/// Join a vector of documents on a separator if the result fits the page,
/// hence the name "smart join", otherwise join them on a line break.
/// Implemented using the LaTeX algorithm described in
/// src/utils.rs
pub fn smart_join<'a>(sep: impl Into<Doc<'a>>, docs: Vec<impl Into<Doc<'a>>>) -> Doc<'a> {
    Doc::SmartJoin(
        Box::new(sep.into()),
        docs.into_iter().map(|d| d.into()).collect(),
    )
}

/// Indent a document by one level.
pub fn indent<'a>(doc: impl Into<Doc<'a>>) -> Doc<'a> {
    Doc::Indent(Box::new(doc.into()))
}

/// Dedent a document by one level.
pub fn dedent<'a>(doc: impl Into<Doc<'a>>) -> Doc<'a> {
    Doc::Dedent(Box::new(doc.into()))
}

/// An absolute line break, i.e. a line break that is always printed.
pub fn hardline<'a>() -> Doc<'a> {
    Doc::Hardline
}

/// A line break, i.e. a line break that is only printed if the document does not fit the page.
pub fn softline<'a>() -> Doc<'a> {
    Doc::Softline
}

/// If the first document fits the page, print it, otherwise print the second document.
pub fn if_break<'a>(doc: Doc<'a>, other: Doc<'a>) -> Doc<'a> {
    Doc::IfBreak(Box::new(doc), Box::new(other))
}

pub trait Group {
    fn group(self) -> Self;
}

impl Group for Doc<'_> {
    fn group(self) -> Self {
        group(self)
    }
}

pub trait Indent {
    fn indent(self) -> Self;
}

impl Indent for Doc<'_> {
    fn indent(self) -> Self {
        indent(self)
    }
}

pub trait Dedent {
    fn dedent(self) -> Self;
}

impl Dedent for Doc<'_> {
    fn dedent(self) -> Self {
        dedent(self)
    }
}

pub trait Join<'a> {
    fn join(self, sep: impl Into<Doc<'a>>) -> Doc<'a>;
}

impl<'a> Join<'a> for Vec<Doc<'a>> {
    fn join(self, sep: impl Into<Doc<'a>>) -> Doc<'a> {
        join(sep, self)
    }
}

pub trait SmartJoin<'a> {
    fn smart_join(self, sep: impl Into<Doc<'a>>) -> Doc<'a>;
}

impl<'a> SmartJoin<'a> for Vec<Doc<'a>> {
    fn smart_join(self, sep: impl Into<Doc<'a>>) -> Doc<'a> {
        smart_join(sep, self)
    }
}

pub trait Wrap<'a> {
    fn wrap(self, left: impl Into<Doc<'a>>, right: impl Into<Doc<'a>>) -> Doc<'a>;
}

impl<'a> Wrap<'a> for Doc<'a> {
    fn wrap(self, left: impl Into<Doc<'a>>, right: impl Into<Doc<'a>>) -> Doc<'a> {
        concat(vec![left.into(), self, right.into()])
    }
}

impl<'a> From<&'a str> for Doc<'a> {
    fn from(s: &'a str) -> Doc<'a> {
        Doc::String(s.into())
    }
}

impl<'a> From<String> for Doc<'a> {
    fn from(s: String) -> Doc<'a> {
        Doc::String(s.into())
    }
}

impl<'a> From<bool> for Doc<'a> {
    fn from(b: bool) -> Doc<'a> {
        Doc::String(b.to_string().into())
    }
}

macro_rules! impl_from_number_to_doc {
    ($($t:ty),*) => {
        $(
            impl<'a> From<$t> for Doc<'a>  {
                fn from(value: $t) -> Self {
                    Doc::String(value.to_string().into())
                }
            }
        )*
    };
}
impl_from_number_to_doc!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

impl<'a, T> From<Option<T>> for Doc<'a>
where
    T: Into<Doc<'a>>,
{
    fn from(opt: Option<T>) -> Doc<'a> {
        match opt {
            Some(value) => value.into(),
            None => Doc::from("None"),
        }
    }
}

impl<'a, T> From<&[T]> for Doc<'a>
where
    T: Into<Doc<'a>> + Clone,
{
    fn from(slice: &[T]) -> Doc<'a> {
        slice
            .iter()
            .map(|item| item.clone().into())
            .collect::<Vec<_>>()
            .into()
    }
}

impl From<()> for Doc<'_> {
    fn from(_: ()) -> Self {
        Doc::from("()")
    }
}

impl<'a, T> From<&T> for Doc<'a>
where
    T: Into<Doc<'a>> + Clone,
{
    fn from(value: &T) -> Self {
        value.clone().into()
    }
}

impl<'a, T> From<Box<T>> for Doc<'a>
where
    T: Into<Doc<'a>>,
{
    fn from(value: Box<T>) -> Self {
        (*value).into()
    }
}

impl<'a> From<Cow<'a, str>> for Doc<'a> {
    fn from(cow: Cow<'a, str>) -> Self {
        match cow {
            Cow::Borrowed(s) => s.into(),
            Cow::Owned(s) => s.into(),
        }
    }
}

impl<'a> From<Regex> for Doc<'a> {
    fn from(regex: Regex) -> Self {
        regex.as_str().to_owned().into()
    }
}

macro_rules! impl_from_tuple_to_doc {
    ($($t:ident),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($t),*> From<($($t),*)> for Doc<'a>
        where
            $($t: Into<Doc<'a>>),*
        {
            fn from(tuple: ($($t),*)) -> Self {
                let ($($t),*) = tuple;
                vec![$($t.into()),*]
                    .smart_join(", ")
                    .group()
                    .wrap("(", ")")
            }
        }
    };
}

impl_from_tuple_to_doc!(T1, T2);
impl_from_tuple_to_doc!(T1, T2, T3);
impl_from_tuple_to_doc!(T1, T2, T3, T4);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6, T7);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_from_tuple_to_doc!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

impl<'a, T> From<Vec<T>> for Doc<'a>
where
    T: Into<Doc<'a>>,
{
    fn from(vec: Vec<T>) -> Doc<'a> {
        let doc_vec: Vec<_> = vec.into_iter().map(|item| item.into()).collect();

        if !doc_vec.is_empty() {
            let doc = doc_vec.smart_join(", ").group().wrap("[", "]").indent();
            doc
        } else {
            Doc::from("[]")
        }
    }
}

impl<'a, K, V, R> From<HashMap<K, V, R>> for Doc<'a>
where
    K: Into<Doc<'a>>,
    V: Into<Doc<'a>>,
{
    fn from(map: HashMap<K, V, R>) -> Doc<'a> {
        let doc_vec: Vec<_> = map
            .into_iter()
            .map(|(key, value)| key.into() + Doc::from(": ") + value.into())
            .collect();

        if !doc_vec.is_empty() {
            let doc = doc_vec
                .join(Doc::from(", ") + Doc::Hardline)
                .group()
                .wrap("{", "}")
                .indent();
            doc
        } else {
            Doc::from("{}")
        }
    }
}

impl<'a, T> From<HashSet<T>> for Doc<'a>
where
    T: Into<Doc<'a>>,
{
    fn from(set: HashSet<T>) -> Self {
        let doc_vec: Vec<_> = set.into_iter().map(|item| item.into()).collect();

        if !doc_vec.is_empty() {
            let doc = doc_vec.smart_join(", ").group().wrap("{", "}").indent();
            doc
        } else {
            Doc::from("{}")
        }
    }
}
