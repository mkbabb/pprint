use std::collections::{HashMap, HashSet};

use crate::builder::FmtBuilder;
use crate::render::{Printer, render_with_hint};

/// Types that can write their pretty-print representation into an FmtBuilder.
///
/// This is the primary formatting trait for pprint. Implement this for your
/// types, or use `#[derive(Pretty)]` to generate it automatically.
///
/// The `'a` lifetime ties `self` to the FmtBuilder, allowing zero-copy
/// `Text(&'a str)` emission for borrowed string fields.
pub trait Pretty<'a> {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>);
}

/// Pretty-print a value to a String.
///
/// ```ignore
/// use pprint::{Pretty, pprint, PRINTER};
/// let v = vec![1, 2, 3];
/// println!("{}", pprint(&v, PRINTER));
/// ```
pub fn pprint<'a>(value: &'a impl Pretty<'a>, printer: Printer) -> String {
    pprint_with_hint(value, printer, 1024)
}

/// Pretty-print with an output buffer size hint.
pub fn pprint_with_hint<'a>(
    value: &'a impl Pretty<'a>,
    printer: Printer,
    size_hint: usize,
) -> String {
    let mut b = FmtBuilder::new();
    value.pretty(&mut b);
    render_with_hint(&b.finish(), printer, size_hint)
}

// ── Primitive impls ─────────────────────────────────────────────────────────

macro_rules! impl_pretty_int {
    ($($t:ty => $method:ident),*) => {
        $(
            impl<'a> Pretty<'a> for $t {
                #[inline]
                fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
                    b.$method(*self as _);
                }
            }
        )*
    };
}

impl_pretty_int!(
    i8 => i64, i16 => i64, i32 => i64, i64 => i64, isize => i64,
    u8 => u64, u16 => u64, u32 => u64, u64 => u64, usize => u64
);

impl<'a> Pretty<'a> for i128 {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.i128(*self);
    }
}

impl<'a> Pretty<'a> for u128 {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.u128(*self);
    }
}

impl<'a> Pretty<'a> for f32 {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.f64(*self as f64);
    }
}

impl<'a> Pretty<'a> for f64 {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.f64(*self);
    }
}

impl<'a> Pretty<'a> for () {
    #[inline]
    fn pretty(&'a self, _b: &mut FmtBuilder<'a>) {}
}

impl<'a> Pretty<'a> for bool {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.text(if *self { "true" } else { "false" });
    }
}

impl<'a> Pretty<'a> for &'a str {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.text(self);
    }
}

impl<'a> Pretty<'a> for String {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.text(self.as_str());
    }
}

impl<'a> Pretty<'a> for std::borrow::Cow<'a, str> {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.text(self.as_ref());
    }
}

// ── Wrapper impls ───────────────────────────────────────────────────────────

impl<'a, T: Pretty<'a>> Pretty<'a> for &'a T {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        (*self).pretty(b);
    }
}

impl<'a, T: Pretty<'a>> Pretty<'a> for Box<T> {
    #[inline]
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        self.as_ref().pretty(b);
    }
}

impl<'a, T: Pretty<'a>> Pretty<'a> for Option<T> {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        match self {
            Some(v) => v.pretty(b),
            None => b.text("None"),
        }
    }
}

// ── Collection impls ────────────────────────────────────────────────────────

/// Helper: emit a list of items with wrap/sep structure.
fn pretty_list<'a>(
    b: &mut FmtBuilder<'a>,
    open: u8,
    close: u8,
    empty: &'a str,
    len: usize,
    mut emit: impl FnMut(&mut FmtBuilder<'a>, usize),
) {
    if len == 0 {
        b.str(empty);
        return;
    }
    b.wrap(open, close, |b| {
        for i in 0..len {
            if i > 0 {
                b.comma_sep();
            }
            emit(b, i);
        }
    });
}

impl<'a, T: Pretty<'a>> Pretty<'a> for Vec<T> {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        pretty_list(b, b'[', b']', "[]", self.len(), |b, i| self[i].pretty(b));
    }
}

impl<'a, T: Pretty<'a>> Pretty<'a> for [T] {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        pretty_list(b, b'[', b']', "[]", self.len(), |b, i| self[i].pretty(b));
    }
}

impl<'a, K: Pretty<'a>, V: Pretty<'a>> Pretty<'a> for HashMap<K, V> {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        let entries: Vec<_> = self.iter().collect();
        pretty_list(b, b'{', b'}', "{}", entries.len(), |b, i| {
            let (k, v) = entries[i];
            k.pretty(b);
            b.str(": ");
            v.pretty(b);
        });
    }
}

impl<'a, T: Pretty<'a>> Pretty<'a> for HashSet<T> {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        let items: Vec<_> = self.iter().collect();
        pretty_list(b, b'{', b'}', "{}", items.len(), |b, i| items[i].pretty(b));
    }
}

// ── Tuple impls ─────────────────────────────────────────────────────────────

macro_rules! impl_pretty_tuple {
    ($($T:ident $idx:tt),+) => {
        impl<'a, $($T: Pretty<'a>),+> Pretty<'a> for ($($T,)+) {
            fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
                b.wrap(b'(', b')', |b| {
                    let mut _first = true;
                    $(
                        if !_first {
                            b.comma_sep();
                        }
                        _first = false;
                        self.$idx.pretty(b);
                    )+
                });
            }
        }
    };
}

impl_pretty_tuple!(T0 0, T1 1);
impl_pretty_tuple!(T0 0, T1 1, T2 2);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5, T6 6);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5, T6 6, T7 7);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5, T6 6, T7 7, T8 8);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5, T6 6, T7 7, T8 8, T9 9);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5, T6 6, T7 7, T8 8, T9 9, T10 10);
impl_pretty_tuple!(T0 0, T1 1, T2 2, T3 3, T4 4, T5 5, T6 6, T7 7, T8 8, T9 9, T10 10, T11 11);

// ── Regex (feature-gated) ───────────────────────────────────────────────────

#[cfg(feature = "regex")]
impl<'a> Pretty<'a> for regex::Regex {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>) {
        b.text(self.as_str());
    }
}
