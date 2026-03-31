use crate::op::FmtOp;
use crate::utils::text_justify;

/// Flat-buffer formatting builder. Constructs a `Vec<FmtOp>` with automatic
/// `GroupStart.flat_width` patching.
///
/// Usage:
/// ```ignore
/// let mut b = FmtBuilder::new();
/// b.group(|b| {
///     b.text("hello");
///     b.softline();
///     b.text("world");
/// });
/// let ops = b.finish();
/// ```
pub struct FmtBuilder<'a> {
    ops: Vec<FmtOp<'a>>,
    /// Stack of (GroupStart index, accumulated flat_width at open).
    group_stack: Vec<(usize, u32)>,
    /// Running flat-mode width since the innermost GroupStart.
    running_width: u32,
}

impl<'a> FmtBuilder<'a> {
    #[inline]
    pub fn new() -> Self {
        Self {
            ops: Vec::with_capacity(256),
            group_stack: Vec::new(),
            running_width: 0,
        }
    }

    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            ops: Vec::with_capacity(cap),
            group_stack: Vec::new(),
            running_width: 0,
        }
    }

    /// Consume the builder and return the op buffer.
    #[inline]
    pub fn finish(mut self) -> Vec<FmtOp<'a>> {
        // Close any unclosed groups (from early returns in failed branches).
        while !self.group_stack.is_empty() {
            self.group_close();
        }
        self.ops
    }

    /// Save a checkpoint (op count + group stack depth) for rollback.
    #[inline]
    pub fn checkpoint(&self) -> (usize, usize, u32) {
        (self.ops.len(), self.group_stack.len(), self.running_width)
    }

    /// Restore to a checkpoint (truncate ops and group stack).
    #[inline]
    pub fn restore(&mut self, (ops_len, groups_len, width): (usize, usize, u32)) {
        self.ops.truncate(ops_len);
        self.group_stack.truncate(groups_len);
        self.running_width = width;
    }

    /// Borrow the ops built so far (for inspection/testing).
    #[inline]
    pub fn ops(&self) -> &[FmtOp<'a>] {
        &self.ops
    }

    // ── Leaf ops ──────────────────────────────────────────────────

    /// Emit borrowed text from the source input.
    #[inline]
    pub fn text(&mut self, s: &'a str) {
        if s.is_empty() {
            return;
        }
        self.running_width += s.len() as u32;
        self.ops.push(FmtOp::Text(s));
    }

    /// Emit a single ASCII byte.
    #[inline]
    pub fn char(&mut self, c: u8) {
        self.running_width += 1;
        self.ops.push(FmtOp::Char(c));
    }

    /// Emit a string literal (for generated separators, keywords, etc.).
    /// Caller must ensure the string lives long enough (typically 'static).
    #[inline]
    pub fn str(&mut self, s: &'a str) {
        if s.is_empty() {
            return;
        }
        self.running_width += s.len() as u32;
        self.ops.push(FmtOp::Text(s));
    }

    // ── Numeric ops ────────────────────────────────────────────

    /// Emit a signed 64-bit integer (rendered inline by the renderer).
    #[inline]
    pub fn i64(&mut self, v: i64) {
        self.running_width += crate::DigitCount::len(&v) as u32;
        self.ops.push(FmtOp::I64(v));
    }

    /// Emit an unsigned 64-bit integer (rendered inline by the renderer).
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.running_width += crate::DigitCount::len(&v) as u32;
        self.ops.push(FmtOp::U64(v));
    }

    /// Emit a signed 128-bit integer.
    #[inline]
    pub fn i128(&mut self, v: i128) {
        self.running_width += crate::DigitCount::len(&v) as u32;
        self.ops.push(FmtOp::I128(v));
    }

    /// Emit an unsigned 128-bit integer.
    #[inline]
    pub fn u128(&mut self, v: u128) {
        self.running_width += crate::DigitCount::len(&v) as u32;
        self.ops.push(FmtOp::U128(v));
    }

    /// Emit a 64-bit float (rendered with Dragonbox by the renderer).
    /// Uses a conservative upper-bound (24) for flat_width estimation.
    #[inline]
    pub fn f64(&mut self, v: f64) {
        self.running_width += 24; // dragonbox max output length
        self.ops.push(FmtOp::F64(v));
    }

    /// Emit a signed integer, widening to i64.
    #[inline]
    pub fn int(&mut self, v: impl Into<i64>) {
        self.i64(v.into());
    }

    /// Emit an unsigned integer, widening to u64.
    #[inline]
    pub fn uint(&mut self, v: impl Into<u64>) {
        self.u64(v.into());
    }

    /// Emit a float, widening to f64.
    #[inline]
    pub fn float(&mut self, v: impl Into<f64>) {
        self.f64(v.into());
    }

    // ── Line ops ─────────────────────────────────────────────────

    /// Unconditional line break.
    #[inline]
    pub fn hardline(&mut self) {
        self.ops.push(FmtOp::Hardline);
        // Hardline contributes max_width to flat_width so that any
        // enclosing Group will always choose break mode.
        self.running_width = self.running_width.saturating_add(10000);
    }

    /// In break mode: newline + indent. In flat mode: single space.
    #[inline]
    pub fn softline(&mut self) {
        self.running_width += 1; // flat-mode width = 1 space
        self.ops.push(FmtOp::Softline);
    }

    /// In break mode: newline + indent. In flat mode: nothing.
    #[inline]
    pub fn break_line(&mut self) {
        self.ops.push(FmtOp::BreakLine);
        // flat-mode width = 0
    }

    /// Line break when `current_line_len > max_width / 2`, otherwise nothing.
    #[inline]
    pub fn mediumline(&mut self) {
        self.ops.push(FmtOp::Mediumline);
        // flat-mode width = 0
    }

    // ── Structural ops ───────────────────────────────────────────

    /// Open a group. Closure `f` emits the group's children.
    /// `flat_width` is automatically patched when the group closes.
    #[inline]
    pub fn group<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.group_open();
        f(self);
        self.group_close();
    }

    /// Open a group (explicit begin — for codegen that can't use closures).
    /// Must be paired with `group_close()`.
    #[inline]
    pub fn group_open(&mut self) {
        let pos = self.ops.len();
        self.ops.push(FmtOp::GroupStart { flat_width: 0 });
        let saved_width = self.running_width;
        self.running_width = 0;
        self.group_stack.push((pos, saved_width));
    }

    /// Close the innermost group, patching GroupStart.flat_width.
    #[inline]
    pub fn group_close(&mut self) {
        let (start_pos, outer_width) = self.group_stack.pop().unwrap();
        let flat_width = self.running_width;
        if let FmtOp::GroupStart { flat_width: ref mut fw } = self.ops[start_pos] {
            *fw = flat_width;
        }
        self.running_width = outer_width + flat_width;
        self.ops.push(FmtOp::GroupEnd);
    }

    /// Open an indent region. Closure `f` emits indented children.
    #[inline]
    pub fn indent<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.indent_open();
        f(self);
        self.indent_close();
    }

    /// Open indent (explicit — for codegen).
    #[inline]
    pub fn indent_open(&mut self) {
        self.ops.push(FmtOp::IndentStart);
    }

    /// Close indent (explicit — for codegen).
    #[inline]
    pub fn indent_close(&mut self) {
        self.ops.push(FmtOp::IndentEnd);
    }

    /// Emit an IfBreak: `break_fn` runs in break mode, `flat_fn` in flat mode.
    /// Both are encoded inline in the op buffer with skip counts.
    /// Only the flat branch contributes to flat_width (for Group decisions).
    pub fn if_break<F1: FnOnce(&mut Self), F2: FnOnce(&mut Self)>(
        &mut self,
        break_fn: F1,
        flat_fn: F2,
    ) {
        let if_pos = self.ops.len();
        self.ops
            .push(FmtOp::IfBreak { break_count: 0, flat_count: 0 }); // placeholder

        // Break side: save+restore running_width so it doesn't affect flat_width.
        let saved_width = self.running_width;
        let break_start = self.ops.len();
        break_fn(self);
        let break_count = (self.ops.len() - break_start) as u16;
        self.running_width = saved_width;

        // Flat side: running_width accumulates normally.
        let flat_start = self.ops.len();
        flat_fn(self);
        let flat_count = (self.ops.len() - flat_start) as u16;

        if let FmtOp::IfBreak {
            break_count: ref mut bc,
            flat_count: ref mut fc,
        } = self.ops[if_pos]
        {
            *bc = break_count;
            *fc = flat_count;
        }
    }

    // ── Join ops ─────────────────────────────────────────────────

    /// Join items with a separator, using the enclosing Group's break mode.
    /// In flat mode: separator between items. In break mode: hardline between items.
    pub fn join<I, F>(&mut self, items: I, sep: F)
    where
        I: IntoIterator,
        I::Item: 'a,
        F: Fn(&mut Self, I::Item),
    {
        let mut first = true;
        for item in items {
            if !first {
                self.if_break(
                    |b| b.hardline(),
                    |b| b.str(", "),
                );
            }
            first = false;
            sep(self, item);
        }
    }

    /// Join items with a separator using SmartJoin (text_justify bin-packing).
    /// Items that fit on the current line are separated by `flat_sep`;
    /// line breaks are inserted at optimal positions.
    pub fn smart_join<I, F>(
        &mut self,
        items: I,
        flat_sep: &'a str,
        max_width: usize,
        first_line_offset: usize,
        mut emit_item: F,
    ) where
        I: IntoIterator,
        I::Item: 'a,
        F: FnMut(&mut Self, I::Item),
    {
        // First pass: emit all items, record their widths.
        let mut item_starts: Vec<usize> = Vec::new();
        let mut item_widths: Vec<usize> = Vec::new();
        let items_start = self.ops.len();

        for item in items {
            let start_width = self.running_width;
            let start_pos = self.ops.len();
            item_starts.push(start_pos);
            emit_item(self, item);
            let item_width = (self.running_width - start_width) as usize;
            item_widths.push(item_width);
        }

        if item_widths.is_empty() {
            return;
        }

        // Compute optimal break positions.
        let sep_width = flat_sep.len();
        let mut breaks = Vec::new();
        text_justify(sep_width, &item_widths, max_width, first_line_offset, &mut breaks);

        // Interleave separators. Drain item ops and rebuild with separators.
        let all_item_ops: Vec<FmtOp<'a>> = self.ops.drain(items_start..).collect();

        // breaks is already sorted — use a cursor walk instead of HashSet.
        let mut break_cursor = 0;

        let n = item_starts.len();
        for i in 0..n {
            let item_range_start = item_starts[i] - items_start;
            let item_range_end = if i + 1 < n {
                item_starts[i + 1] - items_start
            } else {
                all_item_ops.len()
            };
            // Emit item ops.
            self.ops.extend_from_slice(&all_item_ops[item_range_start..item_range_end]);
            // Emit separator (if not last item).
            if i + 1 < n {
                if break_cursor < breaks.len() && breaks[break_cursor] == i + 1 {
                    self.hardline();
                    break_cursor += 1;
                } else {
                    self.str(flat_sep);
                }
            }
        }
    }

    /// Join items with a separator using LinearJoin (greedy line-breaking).
    /// Breaks when the next item wouldn't fit on the current line.
    pub fn linear_join<I, F>(
        &mut self,
        items: I,
        flat_sep: &'a str,
        max_width: usize,
        mut emit_item: F,
    ) where
        I: IntoIterator,
        I::Item: 'a,
        F: FnMut(&mut Self, I::Item),
    {
        let sep_width = flat_sep.len() as u32;
        let mut line_width = 0u32;
        let mut first = true;

        for item in items {
            let start_width = self.running_width;
            emit_item(self, item);
            let item_width = self.running_width - start_width;

            if first {
                line_width = item_width;
                first = false;
            } else {
                let total = line_width + sep_width + item_width;
                if total > max_width as u32 {
                    // Break before this item.
                    self.hardline();
                    line_width = item_width;
                } else {
                    self.str(flat_sep);
                    line_width = total;
                }
            }
        }
    }
}

impl<'a> Default for FmtBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PRINTER, Printer, render};

    fn r(ops: &[FmtOp<'_>], max_width: usize) -> String {
        render(ops, Printer { max_width, ..PRINTER })
    }

    #[test]
    fn test_basic_text() {
        let mut b = FmtBuilder::new();
        b.text("hello world");
        assert_eq!(r(&b.finish(), 80), "hello world");
    }

    #[test]
    fn test_group_fits() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("a");
            b.softline();
            b.text("b");
        });
        assert_eq!(r(&b.finish(), 80), "a b");
    }

    #[test]
    fn test_group_breaks() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("aaaa");
            b.softline();
            b.text("bbbb");
        });
        assert_eq!(r(&b.finish(), 6), "aaaa\nbbbb");
    }

    #[test]
    fn test_indent() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.char(b'{');
            b.indent(|b| {
                b.softline();
                b.text("x");
            });
            b.softline();
            b.char(b'}');
        });
        assert_eq!(r(&b.finish(), 3), "{\n    x\n}");
    }

    #[test]
    fn test_hardline_forces_break() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("a");
            b.hardline();
            b.text("b");
        });
        assert_eq!(r(&b.finish(), 80), "a\nb");
    }

    #[test]
    fn test_nested_groups() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("[");
            b.group(|b| {
                b.text("a");
                b.softline();
                b.text("b");
            });
            b.text("]");
        });
        assert_eq!(r(&b.finish(), 80), "[a b]");
    }

    #[test]
    fn test_if_break() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("aaa");
            b.if_break(
                |b| {
                    b.hardline();
                    b.text("bbb");
                },
                |b| {
                    b.char(b' ');
                    b.text("bbb");
                },
            );
        });
        let ops = b.finish();
        assert_eq!(r(&ops, 80), "aaa bbb");
        assert_eq!(r(&ops, 5), "aaa\nbbb");
    }

    // ── Numeric ops ─────────────────────────────────────────────

    #[test]
    fn test_numeric_i64() {
        let mut b = FmtBuilder::new();
        b.i64(42);
        assert_eq!(r(&b.finish(), 80), "42");

        let mut b = FmtBuilder::new();
        b.i64(-1);
        assert_eq!(r(&b.finish(), 80), "-1");

        let mut b = FmtBuilder::new();
        b.i64(i64::MIN);
        assert_eq!(r(&b.finish(), 80), i64::MIN.to_string());
    }

    #[test]
    fn test_numeric_u64() {
        let mut b = FmtBuilder::new();
        b.u64(0);
        assert_eq!(r(&b.finish(), 80), "0");

        let mut b = FmtBuilder::new();
        b.u64(u64::MAX);
        assert_eq!(r(&b.finish(), 80), u64::MAX.to_string());
    }

    #[test]
    fn test_numeric_i128() {
        let mut b = FmtBuilder::new();
        b.i128(i128::MIN);
        assert_eq!(r(&b.finish(), 80), i128::MIN.to_string());

        let mut b = FmtBuilder::new();
        b.i128(i128::MAX);
        assert_eq!(r(&b.finish(), 80), i128::MAX.to_string());
    }

    #[test]
    fn test_numeric_u128() {
        let mut b = FmtBuilder::new();
        b.u128(u128::MAX);
        assert_eq!(r(&b.finish(), 80), u128::MAX.to_string());
    }

    #[test]
    fn test_numeric_f64() {
        let mut b = FmtBuilder::new();
        b.f64(3.14);
        let out = r(&b.finish(), 80);
        assert!(out.starts_with("3.14"), "got: {out}");
    }

    #[test]
    fn test_numeric_in_group() {
        // Group with numbers that fits
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.i64(1);
            b.softline();
            b.i64(2);
        });
        assert_eq!(r(&b.finish(), 80), "1 2");

        // Group with numbers that doesn't fit → break
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.i64(12345);
            b.softline();
            b.i64(67890);
        });
        assert_eq!(r(&b.finish(), 8), "12345\n67890");
    }

    #[test]
    fn test_mixed_text_numeric() {
        let mut b = FmtBuilder::new();
        b.text("x = ");
        b.i64(42);
        b.text(", y = ");
        b.f64(3.14);
        let out = r(&b.finish(), 80);
        assert!(out.starts_with("x = 42, y = 3.14"), "got: {out}");
    }

    // ── Mediumline ──────────────────────────────────────────────

    #[test]
    fn test_mediumline() {
        // Under half-width: no break
        let mut b = FmtBuilder::new();
        b.text("short");
        b.mediumline();
        b.text("more");
        assert_eq!(r(&b.finish(), 80), "shortmore");

        // Over half-width: break
        let mut b = FmtBuilder::new();
        b.text("this is a long line that exceeds half");
        b.mediumline();
        b.text("next");
        assert_eq!(r(&b.finish(), 40), "this is a long line that exceeds half\nnext");
    }

    // ── Join ops ────────────────────────────────────────────────

    #[test]
    fn test_join_empty() {
        let mut b = FmtBuilder::new();
        let items: Vec<&str> = vec![];
        b.join(items.into_iter(), |b, s| b.text(s));
        assert_eq!(r(&b.finish(), 80), "");
    }

    #[test]
    fn test_join_single() {
        let mut b = FmtBuilder::new();
        b.join(["hello"].into_iter(), |b, s| b.text(s));
        assert_eq!(r(&b.finish(), 80), "hello");
    }

    #[test]
    fn test_smart_join_basic() {
        let items = vec!["aaa", "bbb", "ccc", "ddd"];
        let mut b = FmtBuilder::new();
        b.smart_join(items.into_iter(), ", ", 15, 0, |b, s| b.text(s));
        let out = r(&b.finish(), 80);
        // At width 15: "aaa, bbb, ccc" = 13, fits. "aaa, bbb, ccc, ddd" = 18, breaks.
        assert!(out.contains('\n'), "expected break in: {out}");
    }

    #[test]
    fn test_smart_join_all_fit() {
        let items = vec!["a", "b", "c"];
        let mut b = FmtBuilder::new();
        b.smart_join(items.into_iter(), ", ", 80, 0, |b, s| b.text(s));
        assert_eq!(r(&b.finish(), 80), "a, b, c");
    }

    #[test]
    fn test_linear_join_basic() {
        let items = vec!["aaa", "bbb", "ccc", "ddd"];
        let mut b = FmtBuilder::new();
        b.linear_join(items.into_iter(), ", ", 12, |b, s| b.text(s));
        let out = r(&b.finish(), 80);
        // At width 12: "aaa, bbb" = 8, "aaa, bbb, ccc" = 13 > 12, so break before ccc
        assert!(out.contains('\n'), "expected break in: {out}");
    }

    // ── Deep nesting ────────────────────────────────────────────

    #[test]
    fn test_deep_nesting() {
        let mut b = FmtBuilder::new();
        // 10 nested groups with indent
        for _ in 0..10 {
            b.group_open();
            b.indent_open();
            b.softline();
        }
        b.text("deep");
        for _ in 0..10 {
            b.indent_close();
            b.group_close();
        }
        let out = r(&b.finish(), 3);
        // Should produce deeply indented output
        assert!(out.contains("deep"));
        assert!(out.contains('\n'));
    }

    // ── use_tabs ────────────────────────────────────────────────

    #[test]
    fn test_use_tabs() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.char(b'{');
            b.indent(|b| {
                b.softline();
                b.text("x");
            });
            b.softline();
            b.char(b'}');
        });
        let ops = b.finish();
        let out = render(&ops, Printer { max_width: 3, indent: 1, use_tabs: true });
        assert_eq!(out, "{\n\tx\n}");
    }
}
