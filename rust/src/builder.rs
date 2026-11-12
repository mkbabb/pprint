use crate::layout::{BuildError, compile, f64_width};
use crate::op::{AsciiByte, Doc, FmtOp};
use crate::utils::text_justify;
use crate::width;

/// Flat-buffer formatting builder. Constructs a [`Doc`]: the `FmtOp` stream
/// plus its [`GroupHdr`](crate::GroupHdr) table, which `finish()` computes.
///
/// ```
/// use pprint::{FmtBuilder, PRINTER, render};
/// let mut b = FmtBuilder::new();
/// b.group(|b| {
///     b.text("hello");
///     b.softline();
///     b.text("world");
/// });
/// let doc = b.finish().unwrap();
/// assert_eq!(render(&doc, PRINTER), "hello world");
/// ```
pub struct FmtBuilder<'a> {
    ops: Vec<FmtOp<'a>>,
    /// Stack of (GroupStart index, accumulated flat_width at open).
    group_stack: Vec<(usize, u32)>,
    /// Running flat-mode width since the innermost GroupStart.
    running_width: u32,
    /// The first misuse seen (a close with nothing open, a count past u32).
    /// `finish()` returns it (law L8: refuse, never guess or panic).
    error: Option<BuildError>,
}

impl<'a> FmtBuilder<'a> {
    #[inline]
    pub fn new() -> Self {
        Self {
            ops: Vec::with_capacity(256),
            group_stack: Vec::new(),
            running_width: 0,
            error: None,
        }
    }

    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            ops: Vec::with_capacity(cap),
            group_stack: Vec::new(),
            running_width: 0,
            error: None,
        }
    }

    /// Consume the builder and compile the document: validate the nesting,
    /// build the group-header table, and compute each group's `trailing`
    /// (law L3′) in one backward pass. An unbalanced stream (an unclosed
    /// group or indent, a close with nothing open, an `IfBreak` branch that a
    /// region crosses) is an `Err` (law L8); nothing is closed on the
    /// caller's behalf.
    pub fn finish(self) -> Result<Doc<'a>, BuildError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        compile(self.ops)
    }

    fn refuse(&mut self, reason: &'static str) {
        let pc = self.pc();
        self.refuse_with(BuildError::Unbalanced { pc, reason });
    }

    fn refuse_with(&mut self, e: BuildError) {
        if self.error.is_none() {
            self.error = Some(e);
        }
    }

    /// The index the next op takes (saturated into u32 for error reports).
    fn pc(&self) -> u32 {
        self.ops.len().min(u32::MAX as usize) as u32
    }

    /// Add a leaf's display width (LW3.PP-1b) to the running flat width. A
    /// multi-line leaf adds its last line: its group is `HAS_HARD`, so the
    /// flat width it lands in is never read for a decision.
    #[inline]
    fn add_width(&mut self, w: usize) {
        let w = u32::try_from(w).unwrap_or(u32::MAX);
        self.running_width = self.running_width.saturating_add(w);
    }

    /// The `AsciiByte` for `b`, or a recorded refusal (LW3.PP-1a).
    fn ascii(&mut self, b: u8) -> AsciiByte {
        match AsciiByte::new(b) {
            Some(a) => a,
            None => {
                let pc = self.pc();
                self.refuse_with(BuildError::NotAscii { pc, byte: b });
                AsciiByte::SPACE
            }
        }
    }

    /// Refuse a text leaf that holds a tab (LW3.PP-1a); true if it is clean.
    fn tab_free(&mut self, s: &str) -> bool {
        if s.as_bytes().contains(&b'\t') {
            let pc = self.pc();
            self.refuse_with(BuildError::Tab { pc });
            false
        } else {
            true
        }
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
        if s.is_empty() || !self.tab_free(s) {
            return;
        }
        self.add_width(width::leaf(s).last);
        self.ops.push(FmtOp::Text(s));
    }

    /// Emit a single printable ASCII byte. Any other byte (non-ASCII, a tab,
    /// a control byte) is refused: `finish()` returns
    /// [`BuildError::NotAscii`] (LW3.PP-1a).
    #[inline]
    pub fn char(&mut self, c: u8) {
        let c = self.ascii(c);
        self.running_width = self.running_width.saturating_add(1);
        self.ops.push(FmtOp::Char(c));
    }

    /// Emit text wrapped in double quotes. Single op fast-path for string values.
    #[inline]
    pub fn quoted_text(&mut self, s: &'a str) {
        if !self.tab_free(s) {
            return;
        }
        // The quotes are counted by `width::quoted`.
        self.add_width(width::quoted(s).last);
        self.ops.push(FmtOp::QuotedText(s));
    }

    /// Emit computed/formatted text. The string is copied into an owned `FmtOp`.
    /// Used for hex formatting, Display fallbacks, and other computed values.
    #[inline]
    pub fn text_owned(&mut self, s: &str) {
        if s.is_empty() || !self.tab_free(s) {
            return;
        }
        self.add_width(width::leaf(s).last);
        self.ops.push(FmtOp::OwnedText(s.to_string()));
    }

    /// Emit text only if it is non-empty and contains no newline bytes.
    /// Used by prettify codegen for optional-whitespace (`?w`) boundaries
    /// where structural whitespace is suppressed by `@pretty` hints.
    #[inline]
    pub fn text_inline_ws(&mut self, s: &'a str) {
        if !s.is_empty() && !s.as_bytes().contains(&b'\n') && self.tab_free(s) {
            self.add_width(width::columns(s));
            self.ops.push(FmtOp::Text(s));
        }
    }

    /// Emit a string literal (for generated separators, keywords, etc.).
    /// Caller must ensure the string lives long enough (typically 'static).
    #[inline]
    pub fn str(&mut self, s: &'a str) {
        if s.is_empty() || !self.tab_free(s) {
            return;
        }
        self.add_width(width::leaf(s).last);
        self.ops.push(FmtOp::Text(s));
    }

    // ── Numeric ops ────────────────────────────────────────────

    /// Emit a signed 64-bit integer (rendered inline by the renderer).
    #[inline]
    pub fn i64(&mut self, v: i64) {
        self.running_width = self.running_width.saturating_add(crate::DigitCount::len(&v) as u32);
        self.ops.push(FmtOp::I64(v));
    }

    /// Emit an unsigned 64-bit integer (rendered inline by the renderer).
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.running_width = self.running_width.saturating_add(crate::DigitCount::len(&v) as u32);
        self.ops.push(FmtOp::U64(v));
    }

    /// Emit a signed 128-bit integer (wide-op: emits two consecutive ops).
    #[inline]
    pub fn i128(&mut self, v: i128) {
        self.running_width = self.running_width.saturating_add(crate::DigitCount::len(&v) as u32);
        self.ops.push(FmtOp::I128Hi((v >> 64) as u64));
        self.ops.push(FmtOp::I128Lo(v as u64));
    }

    /// Emit an unsigned 128-bit integer (wide-op: emits two consecutive ops).
    #[inline]
    pub fn u128(&mut self, v: u128) {
        self.running_width = self.running_width.saturating_add(crate::DigitCount::len(&v) as u32);
        self.ops.push(FmtOp::U128Hi((v >> 64) as u64));
        self.ops.push(FmtOp::U128Lo(v as u64));
    }

    /// Emit a 64-bit float (rendered with ryu by the renderer). Its flat
    /// width is the exact printed length, so group decisions are exact.
    #[inline]
    pub fn f64(&mut self, v: f64) {
        self.running_width = self.running_width.saturating_add(f64_width(v) as u32);
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

    /// Unconditional line break. It adds no flat width: `finish()` marks the
    /// enclosing groups `HAS_HARD`, which forces them to break (law L2).
    #[inline]
    pub fn hardline(&mut self) {
        self.ops.push(FmtOp::Hardline);
    }

    /// In break mode: newline + indent. In flat mode: single space.
    #[inline]
    pub fn softline(&mut self) {
        self.running_width = self.running_width.saturating_add(1); // flat-mode width = 1 space
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
        self.ops.push(FmtOp::GroupStart { hdr: 0 });
        let saved_width = self.running_width;
        self.running_width = 0;
        self.group_stack.push((pos, saved_width));
    }

    /// Close the innermost group, recording its flat width in the open
    /// `GroupStart` (u32, saturating; `finish()` moves it to the header).
    /// A close with no open group is refused by `finish()`.
    #[inline]
    pub fn group_close(&mut self) {
        let Some((start_pos, outer_width)) = self.group_stack.pop() else {
            self.refuse("group_close with no open group");
            return;
        };
        let flat_width = self.running_width;
        match self.ops.get_mut(start_pos) {
            Some(FmtOp::GroupStart { hdr }) => *hdr = flat_width,
            _ => {
                self.refuse("group_close after a restore past its group_open");
                return;
            }
        }
        self.running_width = outer_width.saturating_add(flat_width);
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
    ///
    /// `open` and `close` are printable ASCII bytes; any other byte is
    /// refused at `finish()` ([`BuildError::NotAscii`], LW3.PP-1a).
    pub fn wrap<F: FnOnce(&mut Self)>(&mut self, open: u8, close: u8, f: F) {
        let open = self.ascii(open);
        let pos = self.ops.len();
        self.ops.push(FmtOp::WrapStart { open, hdr: 0 });
        let saved_width = self.running_width;
        self.running_width = 1; // the open char
        f(self);
        let flat_width = self.running_width.saturating_add(1); // +1 for close char
        if let Some(FmtOp::WrapStart { hdr, .. }) = self.ops.get_mut(pos) {
            *hdr = flat_width;
        }
        self.running_width = saved_width.saturating_add(flat_width);
        let close = self.ascii(close);
        self.ops.push(FmtOp::WrapEnd { close });
    }

    /// Emit the universal comma separator: `", "` in flat mode, `","` + newline in break mode.
    /// Zero-payload fast path — use this for all standard list/struct separators.
    #[inline]
    pub fn comma_sep(&mut self) {
        self.running_width = self.running_width.saturating_add(2); // flat-mode width of ", "
        self.ops.push(FmtOp::CommaSep);
    }

    /// Emit a custom separator: in break mode emit `brk` + newline + indent;
    /// in flat mode emit `flat`. For non-comma separators.
    ///
    /// `flat` max 8 bytes, `brk` max 4 bytes, every byte printable ASCII.
    /// A longer separator is refused at `finish()`
    /// ([`BuildError::SepTooLong`]), and any other byte too
    /// ([`BuildError::NotAscii`], LW3.PP-1a).
    #[inline]
    pub fn sep(&mut self, flat: &str, brk: &str) {
        if flat.len() > 8 || brk.len() > 4 {
            let pc = self.pc();
            self.refuse_with(BuildError::SepTooLong { pc });
            return;
        }
        let mut flat_buf = [AsciiByte::SPACE; 8];
        for (slot, &b) in flat_buf.iter_mut().zip(flat.as_bytes()) {
            *slot = self.ascii(b);
        }
        let mut brk_buf = [AsciiByte::SPACE; 4];
        for (slot, &b) in brk_buf.iter_mut().zip(brk.as_bytes()) {
            *slot = self.ascii(b);
        }
        self.running_width = self.running_width.saturating_add(flat.len() as u32); // flat-mode width
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
        self.ops.push(FmtOp::IfBreak {
            break_count: 0,
            flat_count: 0,
        }); // placeholder

        // Break side: save+restore running_width so it doesn't affect flat_width.
        let saved_width = self.running_width;
        let break_start = self.ops.len();
        break_fn(self);
        let break_count = self.count_since(break_start);
        self.running_width = saved_width;

        // Flat side: running_width accumulates normally.
        let flat_start = self.ops.len();
        flat_fn(self);
        let flat_count = self.count_since(flat_start);

        match self.ops.get_mut(if_pos) {
            Some(FmtOp::IfBreak {
                break_count: bc,
                flat_count: fc,
            }) => {
                *bc = break_count;
                *fc = flat_count;
            }
            _ => self.refuse("if_break after a restore past its own start"),
        }
    }

    fn count_since(&mut self, start: usize) -> u32 {
        match u32::try_from(self.ops.len().saturating_sub(start)) {
            Ok(c) => c,
            Err(_) => {
                self.refuse("IfBreak branch longer than u32::MAX ops");
                0
            }
        }
    }

    /// A fill separator, [L] `fillSep`'s `softline` generalized to any
    /// separator: `text head <> FillSep k`, where `head` is `sep` without its
    /// trailing spaces and `k` is their count. The `FillSep` op is
    /// `group (flatAlt linebreak (text (spaces k)))`, Leijen's `softline`
    /// for `k = 1` (ruling S8-B): it breaks if and only if the next item
    /// does not fit on the line (law L5), decided at render time at the real
    /// column, so a group in the previous item sees the separator's own
    /// decision (L3′(c)).
    pub fn fill_sep(&mut self, sep: &'a str) {
        let head = sep.trim_end_matches(' ');
        let k = (sep.len() - head.len()) as u32;
        self.str(head);
        self.running_width = self.running_width.saturating_add(k);
        self.ops.push(FmtOp::FillSep { hdr: 0, spaces: k });
    }

    // ── Trivia (LW3.PP-2, law L9) ────────────────────────────────

    /// A line suffix: the leaves `f` emits are held back and printed just
    /// before the next structural newline (or at the end of the document),
    /// and count as zero width. This is how a trailing comment stays at the
    /// end of its line whatever the groups after it decide. `f` may emit
    /// leaves only; a line, group, indent, `IfBreak`, fill separator or more
    /// trivia inside it is refused at `finish()`
    /// ([`BuildError::SuffixContent`]). An empty suffix emits nothing.
    pub fn line_suffix<F: FnOnce(&mut Self)>(&mut self, f: F) {
        let pos = self.ops.len();
        self.ops.push(FmtOp::LineSuffix { len: 0 });
        let saved_width = self.running_width;
        f(self);
        self.running_width = saved_width;
        let len = self.count_since(pos + 1);
        match self.ops.get_mut(pos) {
            Some(FmtOp::LineSuffix { len: l }) if len > 0 => *l = len,
            Some(FmtOp::LineSuffix { .. }) => {
                self.ops.truncate(pos);
            }
            _ => self.refuse("line_suffix after a restore past its own start"),
        }
    }

    /// If a line suffix is pending, a hard line that flushes it; otherwise
    /// nothing (L9).
    #[inline]
    pub fn line_suffix_boundary(&mut self) {
        self.ops.push(FmtOp::LineSuffixBoundary);
    }

    /// Force the enclosing group, and every group around it, to break, with
    /// no output of its own (L2). A trailing line comment is
    /// `line_suffix(comment)` plus `break_parent()`.
    #[inline]
    pub fn break_parent(&mut self) {
        self.ops.push(FmtOp::BreakParent);
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
            let item_width = self.running_width.saturating_sub(start_width) as usize;
            item_widths.push(item_width);
        }

        if item_widths.is_empty() {
            return;
        }

        // Compute optimal break positions.
        let sep_width = flat_sep.len();
        let mut breaks = Vec::new();
        text_justify(
            sep_width,
            &item_widths,
            max_width,
            first_line_offset,
            &mut breaks,
        );

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
            self.ops
                .extend_from_slice(&all_item_ops[item_range_start..item_range_end]);
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

    /// Join items as a fill (law L5): each separator is a [`fill_sep`],
    /// so it breaks if and only if the next item does not fit, decided by the
    /// renderer at the real column and width (`[L] fillSep`).
    ///
    /// [`fill_sep`]: FmtBuilder::fill_sep
    pub fn linear_join<I, F>(&mut self, items: I, flat_sep: &'a str, mut emit_item: F)
    where
        I: IntoIterator,
        I::Item: 'a,
        F: FnMut(&mut Self, I::Item),
    {
        let mut first = true;
        for item in items {
            if !first {
                self.fill_sep(flat_sep);
            }
            first = false;
            emit_item(self, item);
        }
    }
}

impl<'a> Default for FmtBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
