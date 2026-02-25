use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Write;

use regex::Regex;


const BYTES_SIZE: usize = 24;

/// A Document that can be pretty printed
/// Represents the different ways wherein a doc can be printed
#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub enum Doc<'a> {
    Null,

    Char(u8),
    DoubleChar([u8; 2]),
    TripleChar([u8; 3]),
    QuadChar([u8; 4]),

    Bytes(Vec<u8>, usize),
    SmallBytes([u8; BYTES_SIZE], usize),

    String(Cow<'a, str>),

    i8(i8),
    i16(i16),
    i32(i32),
    i64(i64),
    i128(i128),
    isize(isize),
    u8(u8),
    u16(u16),
    u32(u32),
    u64(u64),
    u128(u128),
    usize(usize),

    f32(f32),
    f64(f64),

    DoubleDoc(Box<Doc<'a>>, Box<Doc<'a>>),
    TripleDoc(Box<Doc<'a>>, Box<Doc<'a>>, Box<Doc<'a>>),
    QuadDoc(Box<Doc<'a>>, Box<Doc<'a>>, Box<Doc<'a>>, Box<Doc<'a>>),

    Concat(Vec<Doc<'a>>),

    Group(Box<Doc<'a>>),

    Indent(Box<Doc<'a>>),
    Dedent(Box<Doc<'a>>),

    Join(Box<Doc<'a>>, Vec<Doc<'a>>),
    SmartJoin(Box<Doc<'a>>, Vec<Doc<'a>>),

    IfBreak(Box<Doc<'a>>, Box<Doc<'a>>),

    Trim,

    HardlineDoc(Box<Doc<'a>>),

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

fn format_small_bytes<'a, T>(value: &T) -> Doc<'a>
where
    T: std::fmt::Display,
{
    let mut bytes = [0u8; BYTES_SIZE];

    write!(&mut bytes[..], "{}", value).unwrap();

    let len = bytes.iter().position(|&x| x == 0).unwrap_or(BYTES_SIZE);

    if len == 1 {
        Doc::Char(bytes[0])
    } else if len == 2 {
        Doc::DoubleChar([bytes[0], bytes[1]])
    } else if len == 3 {
        Doc::TripleChar([bytes[0], bytes[1], bytes[2]])
    } else if len == 4 {
        Doc::QuadChar([bytes[0], bytes[1], bytes[2], bytes[3]])
    } else {
        Doc::SmallBytes(bytes, len)
    }
}

pub fn bytes<'a>(value: &[u8], len: Option<usize>) -> Doc<'a> {
    let len = len.unwrap_or(value.len());

    if len == 1 {
        Doc::Char(value[0])
    } else if len == 2 {
        Doc::DoubleChar([value[0], value[1]])
    } else if len == 3 {
        Doc::TripleChar([value[0], value[1], value[2]])
    } else if len == 4 {
        Doc::QuadChar([value[0], value[1], value[2], value[3]])
    } else if len <= BYTES_SIZE {
        let mut bytes = [0u8; BYTES_SIZE];
        bytes[..len].copy_from_slice(value);

        Doc::SmallBytes(bytes, len)
    } else {
        Doc::Bytes(value.into(), len)
    }
}

/// Group a document if it contains a line break.
/// A group is a document that is printed on a single line if it fits the page,
/// otherwise it's printed with line breaks.
pub fn group<'a>(doc: impl Into<Doc<'a>> + Clone) -> Doc<'a> {
    Doc::Group(Box::new(doc.into()))
}

/// Concatenate a vector of documents into a single document.
pub fn concat<'a>(docs: Vec<impl Into<Doc<'a>> + Clone>) -> Doc<'a> {
    match docs.len() {
        0 => Doc::Null,
        1 => docs[0].clone().into(),
        2 => Doc::DoubleDoc(
            Box::new(docs[0].clone().into()),
            Box::new(docs[1].clone().into()),
        ),
        3 => Doc::TripleDoc(
            Box::new(docs[0].clone().into()),
            Box::new(docs[1].clone().into()),
            Box::new(docs[2].clone().into()),
        ),
        _ => Doc::Concat(docs.iter().map(Doc::from).collect()),
    }
}

/// Enwrap a document with two other documents, `left` and `right`.
pub fn wrap<'a>(
    left: impl Into<Doc<'a>>,
    doc: impl Into<Doc<'a>>,
    right: impl Into<Doc<'a>>,
) -> Doc<'a> {
    Doc::TripleDoc(
        Box::new(left.into()),
        Box::new(doc.into()),
        Box::new(right.into()),
    )
}

/// Join a vector of documents on a separator.
pub fn join<'a>(sep: impl Into<Doc<'a>> + Clone, docs: Vec<impl Into<Doc<'a>> + Clone>) -> Doc<'a> {
    Doc::Join(Box::new(sep.into()), docs.iter().map(Doc::from).collect())
}

/// Join a vector of documents on a separator if the result fits the page,
/// hence the name "smart join", otherwise join them on a line break.
///
/// Implemented using the algorithm described in:
/// src/utils.rs
pub fn smart_join<'a>(
    sep: impl Into<Doc<'a>> + Clone,
    docs: Vec<impl Into<Doc<'a>> + Clone>,
) -> Doc<'a> {
    Doc::SmartJoin(Box::new(sep.into()), docs.iter().map(Doc::from).collect())
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
    fn join(self, sep: impl Into<Doc<'a>> + Clone) -> Doc<'a>;
}

impl<'a> Join<'a> for Vec<Doc<'a>> {
    fn join(self, sep: impl Into<Doc<'a>> + Clone) -> Doc<'a> {
        join(sep, self)
    }
}

pub trait SmartJoin<'a> {
    fn smart_join(self, sep: impl Into<Doc<'a>> + Clone) -> Doc<'a>;
}

impl<'a> SmartJoin<'a> for Vec<Doc<'a>> {
    fn smart_join(self, sep: impl Into<Doc<'a>> + Clone) -> Doc<'a> {
        smart_join(sep, self)
    }
}

pub trait Wrap<'a> {
    fn wrap(self, left: impl Into<Doc<'a>> + Clone, right: impl Into<Doc<'a>> + Clone) -> Doc<'a>;
}

impl<'a> Wrap<'a> for Doc<'a> {
    fn wrap(self, left: impl Into<Doc<'a>> + Clone, right: impl Into<Doc<'a>> + Clone) -> Doc<'a> {
        wrap(left, self, right)
    }
}

impl<'a> From<&'a str> for Doc<'a> {
    fn from(s: &'a str) -> Doc<'a> {
        bytes(s.as_bytes(), s.len().into())
    }
}

impl<'a> From<String> for Doc<'a> {
    fn from(s: String) -> Doc<'a> {
        bytes(s.as_bytes(), s.len().into())
    }
}

impl<'a> From<bool> for Doc<'a> {
    fn from(b: bool) -> Doc<'a> {
        format_small_bytes(&b)
    }
}

macro_rules! impl_from_number_to_doc {
    ($($t:ident),*) => {
        $(
            impl<'a> From<$t> for Doc<'a> {
                fn from(value: $t) -> Self {
                    Doc::$t(value)
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
                    .join(", ")
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
    T: Into<Doc<'a>> + 'a + Clone,
{
    fn from(vec: Vec<T>) -> Doc<'a> {
        if !vec.is_empty() {
            // join(Doc::HardlineDoc(Doc::from(", ").into()), vec)
            // join(Doc::from(", "), vec)
            //     .group()
            //     .wrap("[", "]")
            //     .indent()
            // concat(vec)
            smart_join(
                // Doc::DoubleDoc(Doc::from(", ").into(), Doc::Softline.into()),
                Doc::from(", "),
                vec,
            ).group().wrap("[", "]").indent()
            // join(Doc::from(", "), vec)
            // .indent()
            // Doc::Null
            // Doc::Concat(vec.iter().map(|x| Doc::from(x)).collect())
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
            doc_vec
                .join(Doc::from(", ") + Doc::Hardline)
                .group()
                .wrap("{", "}")
                .indent()
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
            doc_vec.join(", ").group().wrap("{", "}").indent()
        } else {
            Doc::from("{}")
        }
    }
}
