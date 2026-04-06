//! [`EmitSink`] implementation for [`FmtBuilder`].
//!
//! Bridges bbnf-emit's grammar-guided emission with pprint's Wadler-Lindig
//! pretty-printing. Grammar-generated traversal code calls `EmitSink` methods;
//! this impl translates them to `FmtOp` operations with automatic `flat_width`
//! tracking.
//!
//! Composite methods (`wrap_open`, `wrap_close`) use the default decomposition
//! for now. `comma_sep` is overridden with pprint's fused zero-payload `CommaSep` op.

use bbnf_emit::EmitSink;
use crate::builder::FmtBuilder;

impl<'a> EmitSink<'a> for FmtBuilder<'a> {
    type Checkpoint = (usize, usize, u32);

    #[inline]
    fn text(&mut self, s: &'a str) {
        FmtBuilder::text(self, s);
    }

    #[inline]
    fn char(&mut self, c: u8) {
        FmtBuilder::char(self, c);
    }

    #[inline]
    fn text_inline_ws(&mut self, s: &'a str) {
        FmtBuilder::text_inline_ws(self, s);
    }

    #[inline]
    fn i64(&mut self, v: i64) {
        FmtBuilder::i64(self, v);
    }

    #[inline]
    fn u64(&mut self, v: u64) {
        FmtBuilder::u64(self, v);
    }

    #[inline]
    fn f64(&mut self, v: f64) {
        FmtBuilder::f64(self, v);
    }

    #[inline]
    fn hardline(&mut self) {
        FmtBuilder::hardline(self);
    }

    #[inline]
    fn softline(&mut self) {
        FmtBuilder::softline(self);
    }

    #[inline]
    fn break_line(&mut self) {
        FmtBuilder::break_line(self);
    }

    #[inline]
    fn group_open(&mut self) {
        FmtBuilder::group_open(self);
    }

    #[inline]
    fn group_close(&mut self) {
        FmtBuilder::group_close(self);
    }

    #[inline]
    fn indent_open(&mut self) {
        FmtBuilder::indent_open(self);
    }

    #[inline]
    fn indent_close(&mut self) {
        FmtBuilder::indent_close(self);
    }

    #[inline]
    fn sep(&mut self, flat: &str, brk: &str) {
        FmtBuilder::sep(self, flat, brk);
    }

    #[inline]
    fn checkpoint(&mut self) -> Self::Checkpoint {
        FmtBuilder::checkpoint(self)
    }

    #[inline]
    fn restore(&mut self, cp: Self::Checkpoint) {
        FmtBuilder::restore(self, cp);
    }

    // wrap_open / wrap_close: use default decomposition (group + char + indent + break).
    // Fused WrapStart/WrapEnd ops require FmtBuilder API additions (tracked for later).

    #[inline]
    fn comma_sep(&mut self) {
        FmtBuilder::comma_sep(self);
    }
}
