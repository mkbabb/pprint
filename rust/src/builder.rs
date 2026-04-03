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

    /// Lightweight checkpoint — saves only ops length and running width.
    /// Skips group_stack, so only safe when the checkpoint/restore span
    /// does not cross `group_open`/`group_close` calls (e.g. prettify codegen
    /// where structural wrapping happens at the rule level).
    #[inline]
    pub fn light_checkpoint(&self) -> (usize, u32) {
        (self.ops.len(), self.running_width)
    }

    /// Restore to a lightweight checkpoint (truncate ops only, no group stack).
    #[inline]
    pub fn light_restore(&mut self, (ops_len, width): (usize, u32)) {
        self.ops.truncate(ops_len);
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

    /// Emit text wrapped in double quotes. Single op fast-path for string values.
    #[inline]
    pub fn quoted_text(&mut self, s: &'a str) {
        self.running_width += s.len() as u32 + 2; // +2 for quotes
        self.ops.push(FmtOp::QuotedText(s));
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

    /// Emit a signed 128-bit integer (wide-op: emits two consecutive ops).
    #[inline]
    pub fn i128(&mut self, v: i128) {
        self.running_width += crate::DigitCount::len(&v) as u32;
        self.ops.push(FmtOp::I128Hi((v >> 64) as u64));
        self.ops.push(FmtOp::I128Lo(v as u64));
    }

    /// Emit an unsigned 128-bit integer (wide-op: emits two consecutive ops).
    #[inline]
    pub fn u128(&mut self, v: u128) {
        self.running_width += crate::DigitCount::len(&v) as u32;
        self.ops.push(FmtOp::U128Hi((v >> 64) as u64));
        self.ops.push(FmtOp::U128Lo(v as u64));
    }

    /// Emit a 64-bit float (rendered with Dragonbox by the renderer).
    /// Uses a conservative upper-bound (24) for flat_width estimation.
    #[inline]
    pub fn f64(&mut self, v: f64) {
        self.running_width += 24; // ryu max output length
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
        self.ops
            .push(FmtOp::GroupStart { flat_width: 0 });
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
            *fw = flat_width as u16;
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

    /// Open a wrapped container: group + open delimiter + indent + break_line.
    /// Closure `f` emits the container's items. Automatically patches `flat_width`.
    pub fn wrap<F: FnOnce(&mut Self)>(&mut self, open: u8, close: u8, f: F) {
        let pos = self.ops.len();
        self.ops
            .push(FmtOp::WrapStart { open, flat_width: 0 });
        let saved_width = self.running_width;
        self.running_width = 1; // the open char
        f(self);
        let flat_width = self.running_width + 1; // +1 for close char
        if let FmtOp::WrapStart {
            flat_width: ref mut fw,
            ..
        } = self.ops[pos]
        {
            *fw = flat_width as u16;
        }
        self.running_width = saved_width + flat_width;
        self.ops.push(FmtOp::WrapEnd { close });
    }

    /// Emit the universal comma separator: `", "` in flat mode, `","` + newline in break mode.
    /// Zero-payload fast path — use this for all standard list/struct separators.
    #[inline]
    pub fn comma_sep(&mut self) {
        self.running_width += 2; // flat-mode width of ", "
        self.ops.push(FmtOp::CommaSep);
    }

    /// Emit a custom separator: in break mode emit `brk` + newline + indent;
    /// in flat mode emit `flat`. For non-comma separators.
    ///
    /// `flat` max 8 bytes, `brk` max 4 bytes.
    #[inline]
    pub fn sep(&mut self, flat: &str, brk: &str) {
        debug_assert!(flat.len() <= 8, "sep flat must be ≤ 8 bytes");
        debug_assert!(brk.len() <= 4, "sep brk must be ≤ 4 bytes");
        let mut flat_buf = [0u8; 8];
        flat_buf[..flat.len()].copy_from_slice(flat.as_bytes());
        let mut brk_buf = [0u8; 4];
        brk_buf[..brk.len()].copy_from_slice(brk.as_bytes());
        self.running_width += flat.len() as u32; // flat-mode width
        self.ops.push(FmtOp::Sep {
            flat: flat_buf,
            flat_len: flat.len() as u8,
            brk: brk_buf,
            brk_len: brk.len() as u8,
        });
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
    /// In flat mode: `", "` between items. In break mode: `","` + newline between items.
    pub fn join<I, F>(&mut self, items: I, emit: F)
    where
        I: IntoIterator,
        I::Item: 'a,
        F: Fn(&mut Self, I::Item),
    {
        let mut first = true;
        for item in items {
            if !first {
                self.comma_sep();
            }
            first = false;
            emit(self, item);
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
