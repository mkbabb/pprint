use crate::op::{Doc, FmtOp, GroupHdr};

/// Printer configuration.
#[derive(Debug, Clone, Copy)]
pub struct Printer {
    pub max_width: usize,
    pub indent: usize,
    pub use_tabs: bool,
}

/// Default printer configuration: 80 columns, 4-space indent, no tabs.
pub const PRINTER: Printer = Printer {
    max_width: 80,
    indent: 4,
    use_tabs: false,
};

impl Default for Printer {
    fn default() -> Self {
        PRINTER
    }
}

impl Printer {
    pub const fn new(max_width: usize, indent: usize, use_tabs: bool) -> Self {
        Printer {
            max_width,
            indent,
            use_tabs,
        }
    }
}

/// Linear-scan renderer. One forward pass over the op stream, O(n) time,
/// no allocation beyond the output buffer and the indent run.
///
/// A group decision is O(1): flat if and only if the group is not
/// `HAS_HARD` and `column + flat_width + trailing <= max_width`, with both
/// widths precomputed by `FmtBuilder::finish()` ([W] §4 `better`/`fits` with
/// the rest lookahead; laws L1–L3′). Inside a flat group every nested group
/// is flat ([W] §3 `flatten (x :<|> y) = flatten x`).
pub fn render(doc: &Doc<'_>, printer: Printer) -> String {
    render_with_hint(doc, printer, 0)
}

/// Like `render`, but with a size hint for the output buffer.
pub fn render_with_hint(doc: &Doc<'_>, printer: Printer, size_hint: usize) -> String {
    let indent_ch = if printer.use_tabs { b'\t' } else { b' ' };

    // Pre-size output buffer from root op's flat_width or size hint.
    let capacity = if size_hint > 0 {
        size_hint
    } else {
        match doc.ops.first() {
            Some(FmtOp::WrapStart { hdr, .. } | FmtOp::GroupStart { hdr }) => {
                (doc.hdrs[*hdr as usize].flat_width as usize)
                    .saturating_mul(2)
                    .clamp(256, 1 << 20)
            }
            _ => doc.ops.len() * 4,
        }
    };

    let mut state = RenderState {
        output: Vec::with_capacity(capacity),
        group_bits: 0,
        group_depth: 0,
        indent: 0,
        current_line_len: 0,
        // ABI v2: widths are u32, and `max_width` is clamped to u32::MAX / 2.
        max_width: printer.max_width.min(u32::MAX as usize / 2),
        indent_size: printer.indent,
        indent_ch,
        indent_cache: vec![indent_ch; 128],
    };

    state.run(&doc.ops, &doc.hdrs, 0, doc.ops.len());

    // Strip trailing whitespace.
    while state
        .output
        .last()
        .is_some_and(|&b| b == b' ' || b == b'\t')
    {
        state.output.pop();
    }

    // SAFETY: all ops emit valid UTF-8 (Text borrows &str, Char emits ASCII,
    // numeric ops produce ASCII digits).
    unsafe { String::from_utf8_unchecked(state.output) }
}

struct RenderState {
    output: Vec<u8>,
    /// Bitfield group stack: bit N = break mode at depth N. Supports 64 nesting levels.
    group_bits: u64,
    group_depth: u32,
    indent: usize,
    current_line_len: usize,
    max_width: usize,
    indent_size: usize,
    indent_ch: u8,
    indent_cache: Vec<u8>,
}

impl RenderState {
    #[inline(always)]
    fn in_break_mode(&self) -> bool {
        if self.group_depth == 0 {
            return false;
        }
        // Beyond 64 levels: conservatively assume break mode.
        if self.group_depth > 64 {
            return true;
        }
        (self.group_bits >> (self.group_depth - 1)) & 1 == 1
    }

    #[inline(always)]
    fn push_group(&mut self, breaks: bool) {
        if self.group_depth < 64 {
            let bit = (breaks as u64) << self.group_depth;
            self.group_bits = (self.group_bits & !(1u64 << self.group_depth)) | bit;
        }
        self.group_depth += 1;
    }

    #[inline(always)]
    fn pop_group(&mut self) {
        self.group_depth = self.group_depth.saturating_sub(1);
    }

    /// L1/L2/L3′: does the group with header `h` break here? A group nested
    /// in a flat group is flat; elsewhere (in a broken group, or at depth 0)
    /// it breaks unless its flattening fits with its trailing rest.
    #[inline(always)]
    fn decide(&self, h: &GroupHdr) -> bool {
        if self.group_depth > 0 && !self.in_break_mode() {
            return false;
        }
        h.has_hard()
            || self.current_line_len as u64 + h.flat_width as u64 + h.trailing as u64
                > self.max_width as u64
    }

    #[inline(always)]
    fn text(&mut self, s: &[u8]) {
        self.output.extend_from_slice(s);
        self.current_line_len += s.len();
    }

    #[inline(always)]
    fn newline(&mut self) {
        // Strip trailing whitespace before the break.
        while self.output.last().is_some_and(|&b| b == b' ' || b == b'\t') {
            self.output.pop();
        }
        self.output.push(b'\n');
        // Emit indent from cache (single extend_from_slice instead of per-byte push).
        if self.indent > self.indent_cache.len() {
            self.indent_cache.resize(self.indent, self.indent_ch);
        }
        self.output
            .extend_from_slice(&self.indent_cache[..self.indent]);
        self.current_line_len = self.indent;
    }

    /// Render `ops[lo..hi]`. An `IfBreak` renders the branch its mode takes
    /// (law L6) by recursing on that branch's range.
    fn run(&mut self, ops: &[FmtOp<'_>], hdrs: &[GroupHdr], lo: usize, hi: usize) {
        let mut pc = lo;
        while pc < hi {
            match ops[pc] {
                FmtOp::Text(s) => self.text(s.as_bytes()),
                FmtOp::OwnedText(ref s) => self.text(s.as_bytes()),
                FmtOp::QuotedText(s) => {
                    self.output.push(b'"');
                    self.output.extend_from_slice(s.as_bytes());
                    self.output.push(b'"');
                    self.current_line_len += s.len() + 2;
                }
                FmtOp::Char(c) => {
                    self.output.push(c);
                    self.current_line_len += 1;
                }

                FmtOp::I64(v) => emit_int(v, self),
                FmtOp::U64(v) => emit_int(v, self),

                FmtOp::I128Hi(hi_bits) => {
                    // Wide-op: reconstruct i128 from hi + next op (I128Lo)
                    pc += 1;
                    if let Some(&FmtOp::I128Lo(lo_bits)) = ops.get(pc) {
                        emit_int(((hi_bits as i128) << 64) | (lo_bits as i128), self);
                    }
                }
                FmtOp::U128Hi(hi_bits) => {
                    pc += 1;
                    if let Some(&FmtOp::U128Lo(lo_bits)) = ops.get(pc) {
                        emit_int(((hi_bits as u128) << 64) | (lo_bits as u128), self);
                    }
                }
                // Only reachable after its high op, which consumed it.
                FmtOp::I128Lo(_) | FmtOp::U128Lo(_) => {}

                FmtOp::F64(v) => emit_f64(v, self),

                FmtOp::GroupStart { hdr } => {
                    let breaks = self.decide(&hdrs[hdr as usize]);
                    self.push_group(breaks);
                }
                FmtOp::GroupEnd => self.pop_group(),

                FmtOp::IndentStart => self.indent += self.indent_size,
                FmtOp::IndentEnd => {
                    self.indent = self.indent.saturating_sub(self.indent_size);
                }

                FmtOp::WrapStart { open, hdr } => {
                    let breaks = self.decide(&hdrs[hdr as usize]);
                    self.push_group(breaks);
                    self.output.push(open);
                    self.current_line_len += 1;
                    self.indent += self.indent_size;
                    if breaks {
                        self.newline();
                    }
                }
                FmtOp::WrapEnd { close } => {
                    self.indent = self.indent.saturating_sub(self.indent_size);
                    if self.in_break_mode() {
                        self.newline();
                    }
                    self.output.push(close);
                    self.current_line_len += 1;
                    self.pop_group();
                }

                FmtOp::Hardline => self.newline(),
                FmtOp::Softline => {
                    if self.in_break_mode() {
                        self.newline();
                    } else {
                        self.output.push(b' ');
                        self.current_line_len += 1;
                    }
                }
                FmtOp::BreakLine => {
                    if self.in_break_mode() {
                        self.newline();
                    }
                }
                FmtOp::CommaSep => {
                    if self.in_break_mode() {
                        self.output.push(b',');
                        self.current_line_len += 1;
                        self.newline();
                    } else {
                        self.text(b", ");
                    }
                }
                FmtOp::Sep {
                    flat,
                    flat_len,
                    brk,
                    brk_len,
                } => {
                    if self.in_break_mode() {
                        self.text(&brk[..brk_len as usize]);
                        self.newline();
                    } else {
                        self.text(&flat[..flat_len as usize]);
                    }
                }

                FmtOp::IfBreak {
                    break_count,
                    flat_count,
                } => {
                    let flat_start = pc + 1 + break_count as usize;
                    let rest = flat_start + flat_count as usize;
                    if self.in_break_mode() {
                        self.run(ops, hdrs, pc + 1, flat_start);
                    } else {
                        self.run(ops, hdrs, flat_start, rest);
                    }
                    pc = rest;
                    continue;
                }
            }
            pc += 1;
        }
    }
}

#[inline(always)]
fn emit_int<T: itoap::Integer>(value: T, state: &mut RenderState) {
    let prev_len = state.output.len();
    itoap::write_to_vec(&mut state.output, value);
    state.current_line_len += state.output.len() - prev_len;
}

#[inline(always)]
fn emit_f64(value: f64, state: &mut RenderState) {
    let mut buf = ryu::Buffer::new();
    state.text(buf.format_finite(value).as_bytes());
}
