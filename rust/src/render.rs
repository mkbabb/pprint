use crate::op::{AsciiByte, Doc, FmtOp, GroupHdr};
use crate::width;

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

/// Linear-scan renderer. One forward loop over the op stream, O(n) time, no
/// recursion, no allocation beyond the output buffer, the indent run and two
/// stacks that grow with nesting depth (sk-v27 LW3.PP-1a).
///
/// A group decision is O(1): flat if and only if the group is not
/// `HAS_HARD` and `column + flat_width + trailing <= max_width`, with both
/// widths precomputed by `FmtBuilder::finish()` ([W] §4 `better`/`fits` with
/// the rest lookahead; laws L1–L3′). Inside a flat group every nested group
/// is flat ([W] §3 `flatten (x :<|> y) = flatten x`), at any depth: the mode
/// stack is a growable bit vector with no depth cliff.
///
/// The output is built only from `&str` and [`AsciiByte`] pushes, so it is
/// valid UTF-8 by construction, with no unchecked conversion.
///
/// [`AsciiByte`]: crate::AsciiByte
pub fn render(doc: &Doc<'_>, printer: Printer) -> String {
    render_with_hint(doc, printer, 0)
}

/// Like `render`, but with a size hint for the output buffer.
pub fn render_with_hint(doc: &Doc<'_>, printer: Printer, size_hint: usize) -> String {
    let indent_ch = if printer.use_tabs { '\t' } else { ' ' };

    // Pre-size output buffer from root op's flat_width or size hint.
    let capacity = if size_hint > 0 {
        size_hint
    } else {
        match doc.ops.first() {
            Some(FmtOp::WrapStart { hdr, .. } | FmtOp::GroupStart { hdr }) => doc
                .hdrs
                .get(*hdr as usize)
                .map_or(256, |h| (h.flat_width as usize).saturating_mul(2))
                .clamp(256, 1 << 20),
            _ => doc.ops.len().saturating_mul(4),
        }
    };

    let mut state = RenderState {
        output: String::with_capacity(capacity),
        modes: ModeStack::default(),
        indent: 0,
        current_line_len: 0,
        // ABI v2: widths are u32, and `max_width` is clamped to u32::MAX / 2.
        max_width: printer.max_width.min(u32::MAX as usize / 2),
        indent_size: printer.indent,
        indent_ch,
        indent_cache: String::new(),
    };

    state.run(&doc.ops, &doc.hdrs);

    // Strip trailing whitespace.
    state.trim_trailing();
    state.output
}

/// The group mode stack: bit `d` is set iff the group at depth `d` (0-based)
/// is in break mode. A growable bit vector, so no nesting depth reads a
/// wrong mode (L1 at any depth; the u64 stack of LW2 read every group past
/// depth 64 as broken).
#[derive(Default)]
struct ModeStack {
    words: Vec<u64>,
    depth: usize,
}

impl ModeStack {
    #[inline(always)]
    fn push(&mut self, breaks: bool) {
        let (w, b) = (self.depth / 64, self.depth % 64);
        if w == self.words.len() {
            self.words.push(0);
        }
        let mask = 1u64 << b;
        if breaks {
            self.words[w] |= mask;
        } else {
            self.words[w] &= !mask;
        }
        self.depth += 1;
    }

    #[inline(always)]
    fn pop(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// The innermost group's mode; false at depth 0 (pprint's convention:
    /// lines outside every group are flat).
    #[inline(always)]
    fn in_break(&self) -> bool {
        match self.depth.checked_sub(1) {
            None => false,
            Some(d) => (self.words[d / 64] >> (d % 64)) & 1 == 1,
        }
    }
}

struct RenderState {
    output: String,
    modes: ModeStack,
    indent: usize,
    current_line_len: usize,
    max_width: usize,
    indent_size: usize,
    indent_ch: char,
    /// A run of `indent_ch`, grown on demand; a newline copies a prefix.
    indent_cache: String,
}

impl RenderState {
    #[inline(always)]
    fn in_break_mode(&self) -> bool {
        self.modes.in_break()
    }

    /// L1/L2/L3′: does the group with header `h` break here? A group nested
    /// in a flat group is flat; elsewhere (in a broken group, or at depth 0)
    /// it breaks unless its flattening fits with its trailing rest.
    #[inline(always)]
    fn decide(&self, h: &GroupHdr) -> bool {
        if self.modes.depth > 0 && !self.in_break_mode() {
            return false;
        }
        h.has_hard()
            || self.current_line_len as u64 + h.flat_width as u64 + h.trailing as u64
                > self.max_width as u64
    }

    /// Emit a text leaf, counting display columns (LW3.PP-1b). Each line
    /// feed inside the leaf is a newline at column 0: trailing spaces before
    /// it are removed, as before every newline, and no indentation follows
    /// it. The column after the leaf is its last line's width.
    #[inline(always)]
    fn text(&mut self, s: &str) {
        if !s.as_bytes().contains(&b'\n') {
            self.output.push_str(s);
            self.current_line_len += width::columns(s);
            return;
        }
        for (i, line) in s.split('\n').enumerate() {
            if i > 0 {
                self.trim_trailing();
                self.output.push('\n');
                self.current_line_len = 0;
            }
            self.output.push_str(line);
            self.current_line_len += width::columns(line);
        }
    }

    /// An ASCII-only string (numbers, fixed separators): one column a byte.
    #[inline(always)]
    fn ascii_str(&mut self, s: &str) {
        debug_assert!(s.is_ascii());
        self.output.push_str(s);
        self.current_line_len += s.len();
    }

    #[inline(always)]
    fn ascii(&mut self, c: AsciiByte) {
        self.output.push(c.as_char());
        self.current_line_len += 1;
    }

    #[inline(always)]
    fn ascii_run(&mut self, bytes: &[AsciiByte]) {
        for &c in bytes {
            self.output.push(c.as_char());
        }
        self.current_line_len += bytes.len();
    }

    /// Remove trailing spaces and tabs from the output (pprint's convention,
    /// before every newline and at the end).
    #[inline(always)]
    fn trim_trailing(&mut self) {
        let keep = self.output.trim_end_matches([' ', '\t']).len();
        self.output.truncate(keep);
    }

    #[inline(always)]
    fn newline(&mut self) {
        self.trim_trailing();
        self.output.push('\n');
        // Emit indent from cache (one push_str instead of per-char pushes).
        // `indent_ch` is one byte, so a byte prefix of the run is a char
        // boundary.
        if self.indent > self.indent_cache.len() {
            let grow = self.indent - self.indent_cache.len();
            self.indent_cache
                .extend(std::iter::repeat_n(self.indent_ch, grow));
        }
        self.output.push_str(&self.indent_cache[..self.indent]);
        self.current_line_len = self.indent;
    }

    /// Render the whole op stream in one loop. An `IfBreak` renders the
    /// branch its mode takes (law L6): the loop jumps into that branch and
    /// records, on an explicit stack, where the branch ends and where the
    /// stream resumes after the whole `IfBreak`. No recursion, so any nesting
    /// depth renders at a constant native stack.
    fn run(&mut self, ops: &[FmtOp<'_>], hdrs: &[GroupHdr]) {
        // Open IfBreak branches, innermost last: (branch end, resume pc).
        let mut branches: Vec<(usize, usize)> = Vec::new();
        let mut pc = 0usize;
        loop {
            // Leave every branch that ends here.
            while let Some(&(end, resume)) = branches.last() {
                if pc < end {
                    break;
                }
                branches.pop();
                pc = resume;
            }
            let Some(op) = ops.get(pc) else {
                break;
            };
            match *op {
                FmtOp::Text(s) => self.text(s),
                FmtOp::OwnedText(ref s) => self.text(s),
                FmtOp::QuotedText(s) => {
                    self.output.push('"');
                    self.current_line_len += 1;
                    self.text(s);
                    self.output.push('"');
                    self.current_line_len += 1;
                }
                FmtOp::Char(c) => self.ascii(c),

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
                    self.modes.push(breaks);
                }
                FmtOp::GroupEnd => self.modes.pop(),

                FmtOp::IndentStart => self.indent += self.indent_size,
                FmtOp::IndentEnd => {
                    self.indent = self.indent.saturating_sub(self.indent_size);
                }

                FmtOp::WrapStart { open, hdr } => {
                    let breaks = self.decide(&hdrs[hdr as usize]);
                    self.modes.push(breaks);
                    self.ascii(open);
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
                    self.ascii(close);
                    self.modes.pop();
                }

                FmtOp::Hardline => self.newline(),
                FmtOp::Softline => {
                    if self.in_break_mode() {
                        self.newline();
                    } else {
                        self.output.push(' ');
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
                        self.output.push(',');
                        self.current_line_len += 1;
                        self.newline();
                    } else {
                        self.ascii_str(", ");
                    }
                }
                FmtOp::Sep {
                    ref flat,
                    flat_len,
                    ref brk,
                    brk_len,
                } => {
                    if self.in_break_mode() {
                        self.ascii_run(&brk[..(brk_len as usize).min(brk.len())]);
                        self.newline();
                    } else {
                        self.ascii_run(&flat[..(flat_len as usize).min(flat.len())]);
                    }
                }

                FmtOp::IfBreak {
                    break_count,
                    flat_count,
                } => {
                    let flat_start = pc + 1 + break_count as usize;
                    let rest = flat_start + flat_count as usize;
                    let (lo, hi) = if self.in_break_mode() {
                        (pc + 1, flat_start)
                    } else {
                        (flat_start, rest)
                    };
                    branches.push((hi, rest));
                    pc = lo;
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
    itoap::write_to_string(&mut state.output, value);
    state.current_line_len += state.output.len() - prev_len;
}

#[inline(always)]
fn emit_f64(value: f64, state: &mut RenderState) {
    let mut buf = ryu::Buffer::new();
    state.ascii_str(buf.format_finite(value));
}
