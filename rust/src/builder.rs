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

        // Now interleave separators. We need to insert ops BETWEEN items.
        // Rather than shifting the Vec, rebuild with separators.
        let all_item_ops: Vec<FmtOp<'a>> = self.ops.drain(items_start..).collect();

        let mut break_set: std::collections::HashSet<usize> = breaks.into_iter().collect();

        let n = item_starts.len();
        for i in 0..n {
            let item_range_start = item_starts[i] - items_start;
            let item_range_end = if i + 1 < n {
                item_starts[i + 1] - items_start
            } else {
                all_item_ops.len()
            };
            // Emit item ops.
            for op in &all_item_ops[item_range_start..item_range_end] {
                self.ops.push(*op);
            }
            // Emit separator (if not last item).
            if i + 1 < n {
                if break_set.contains(&(i + 1)) {
                    self.hardline();
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
    use crate::render::render;

    #[test]
    fn test_basic_text() {
        let mut b = FmtBuilder::new();
        b.text("hello world");
        let ops = b.finish();
        assert_eq!(render(&ops, 80, 4), "hello world");
    }

    #[test]
    fn test_group_fits() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("a");
            b.softline();
            b.text("b");
        });
        let ops = b.finish();
        // Fits on one line → softline = space
        assert_eq!(render(&ops, 80, 4), "a b");
    }

    #[test]
    fn test_group_breaks() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("aaaa");
            b.softline();
            b.text("bbbb");
        });
        let ops = b.finish();
        // Doesn't fit on 6-wide line → softline = newline
        assert_eq!(render(&ops, 6, 4), "aaaa\nbbbb");
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
        let ops = b.finish();
        // Width 3 → breaks
        assert_eq!(render(&ops, 3, 4), "{\n    x\n}");
    }

    #[test]
    fn test_hardline_forces_break() {
        let mut b = FmtBuilder::new();
        b.group(|b| {
            b.text("a");
            b.hardline();
            b.text("b");
        });
        let ops = b.finish();
        // Hardline always breaks regardless of width
        assert_eq!(render(&ops, 80, 4), "a\nb");
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
        let ops = b.finish();
        // Outer group fits → inner group also flat
        assert_eq!(render(&ops, 80, 4), "[a b]");
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
        // Fits → flat branch
        assert_eq!(render(&ops, 80, 4), "aaa bbb");
        // Doesn't fit → break branch
        assert_eq!(render(&ops, 5, 4), "aaa\nbbb");
    }
}
