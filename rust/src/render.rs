use crate::op::FmtOp;

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

/// Linear-scan renderer. Single pass over the op buffer, O(n) time,
/// zero allocations beyond the output buffer.
///
/// Break decisions are O(1) per Group — `flat_width` is pre-computed
/// by the `FmtBuilder` during construction.
pub fn render(ops: &[FmtOp<'_>], printer: Printer) -> String {
    render_with_hint(ops, printer, 0)
}

/// Like `render`, but with a size hint for the output buffer.
pub fn render_with_hint(
    ops: &[FmtOp<'_>],
    printer: Printer,
    size_hint: usize,
) -> String {
    let indent_ch = if printer.use_tabs { b'\t' } else { b' ' };
    let mut indent_cache: Vec<u8> = vec![indent_ch; 128.min(printer.max_width)];

    // Pre-size output buffer from root op's flat_width or size hint.
    let capacity = if size_hint > 0 {
        size_hint
    } else {
        match ops.first() {
            Some(FmtOp::WrapStart { flat_width, .. } | FmtOp::GroupStart { flat_width }) => {
                (*flat_width as usize).saturating_mul(2).max(256)
            }
            _ => ops.len() * 4,
        }
    };

    let mut state = RenderState {
        output: Vec::with_capacity(capacity),
        group_bits: 0,
        group_depth: 0,
        indent: 0,
        current_line_len: 0,
        max_width: printer.max_width,
        indent_size: printer.indent,
    };

    let mut pc = 0;
    while pc < ops.len() {
        match ops[pc] {
            FmtOp::Text(s) => {
                state.output.extend_from_slice(s.as_bytes());
                state.current_line_len += s.len();
            }

            FmtOp::OwnedText(ref s) => {
                state.output.extend_from_slice(s.as_bytes());
                state.current_line_len += s.len();
            }

            FmtOp::QuotedText(s) => {
                state.output.push(b'"');
                state.output.extend_from_slice(s.as_bytes());
                state.output.push(b'"');
                state.current_line_len += s.len() + 2;
            }

            FmtOp::Char(c) => {
                state.output.push(c);
                state.current_line_len += 1;
            }

            FmtOp::I64(v) => emit_int(v, &mut state),
            FmtOp::U64(v) => emit_int(v, &mut state),

            FmtOp::I128Hi(hi) => {
                // Wide-op: reconstruct i128 from hi + next op (I128Lo)
                pc += 1;
                if let FmtOp::I128Lo(lo) = ops[pc] {
                    let val = ((hi as i128) << 64) | (lo as i128);
                    emit_int(val, &mut state);
                }
            }

            FmtOp::U128Hi(hi) => {
                pc += 1;
                if let FmtOp::U128Lo(lo) = ops[pc] {
                    let val = ((hi as u128) << 64) | (lo as u128);
                    emit_int(val, &mut state);
                }
            }

            FmtOp::I128Lo(_) | FmtOp::U128Lo(_) => {
                // Should only be reached after I128Hi/U128Hi — skip if orphaned.
            }

            FmtOp::F64(v) => emit_f64(v, &mut state),

            FmtOp::GroupStart { flat_width } => {
                let breaks = state.current_line_len + flat_width as usize > state.max_width;
                state.push_group(breaks);
            }

            FmtOp::GroupEnd => {
                state.pop_group();
            }

            FmtOp::IndentStart => {
                state.indent += state.indent_size;
            }

            FmtOp::IndentEnd => {
                state.indent = state.indent.saturating_sub(state.indent_size);
            }

            FmtOp::WrapStart { open, flat_width } => {
                let breaks = state.current_line_len + flat_width as usize > state.max_width;
                state.push_group(breaks);
                state.output.push(open);
                state.current_line_len += 1;
                state.indent += state.indent_size;
                if breaks {
                    emit_newline(&mut state, &mut indent_cache);
                }
            }

            FmtOp::WrapEnd { close } => {
                state.indent = state.indent.saturating_sub(state.indent_size);
                if state.in_break_mode() {
                    emit_newline(&mut state, &mut indent_cache);
                }
                state.output.push(close);
                state.current_line_len += 1;
                state.pop_group();
            }

            FmtOp::Hardline => {
                emit_newline(&mut state, &mut indent_cache);
            }

            FmtOp::Softline => {
                if state.in_break_mode() {
                    emit_newline(&mut state, &mut indent_cache);
                } else {
                    state.output.push(b' ');
                    state.current_line_len += 1;
                }
            }

            FmtOp::BreakLine => {
                if state.in_break_mode() {
                    emit_newline(&mut state, &mut indent_cache);
                }
            }

            FmtOp::Mediumline => {
                if state.current_line_len > state.max_width / 2 {
                    emit_newline(&mut state, &mut indent_cache);
                }
            }

            FmtOp::CommaSep => {
                if state.in_break_mode() {
                    state.output.push(b',');
                    state.current_line_len += 1;
                    emit_newline(&mut state, &mut indent_cache);
                } else {
                    state.output.extend_from_slice(b", ");
                    state.current_line_len += 2;
                }
            }

            FmtOp::Sep {
                flat,
                flat_len,
                brk,
                brk_len,
            } => {
                if state.in_break_mode() {
                    state
                        .output
                        .extend_from_slice(&brk[..brk_len as usize]);
                    state.current_line_len += brk_len as usize;
                    emit_newline(&mut state, &mut indent_cache);
                } else {
                    state
                        .output
                        .extend_from_slice(&flat[..flat_len as usize]);
                    state.current_line_len += flat_len as usize;
                }
            }

            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                if state.in_break_mode() {
                    let break_end = pc + 1 + break_count as usize;
                    for op in &ops[pc + 1..break_end] {
                        render_single_op(op, &mut state, &mut indent_cache);
                    }
                    pc = break_end + flat_count as usize;
                    continue;
                } else {
                    let flat_start = pc + 1 + break_count as usize;
                    let flat_end = flat_start + flat_count as usize;
                    for op in &ops[flat_start..flat_end] {
                        render_single_op(op, &mut state, &mut indent_cache);
                    }
                    pc = flat_end;
                    continue;
                }
            }
        }

        pc += 1;
    }

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
    group_depth: u8,
    indent: usize,
    current_line_len: usize,
    max_width: usize,
    indent_size: usize,
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
    let s = buf.format_finite(value).as_bytes();
    state.output.extend_from_slice(s);
    state.current_line_len += s.len();
}

#[inline(always)]
fn emit_newline(state: &mut RenderState, indent_cache: &mut Vec<u8>) {
    // Strip trailing whitespace before the break.
    while state
        .output
        .last()
        .is_some_and(|&b| b == b' ' || b == b'\t')
    {
        state.output.pop();
    }
    state.output.push(b'\n');
    // Emit indent from cache (single extend_from_slice instead of per-byte push).
    if state.indent > indent_cache.len() {
        indent_cache.resize(state.indent, indent_cache[0]);
    }
    state
        .output
        .extend_from_slice(&indent_cache[..state.indent]);
    state.current_line_len = state.indent;
}

/// Render a single op (used by IfBreak to process sub-ranges).
#[inline(always)]
fn render_single_op(op: &FmtOp<'_>, state: &mut RenderState, indent_cache: &mut Vec<u8>) {
    match op {
        FmtOp::Text(s) => {
            state.output.extend_from_slice(s.as_bytes());
            state.current_line_len += s.len();
        }
        FmtOp::OwnedText(s) => {
            state.output.extend_from_slice(s.as_bytes());
            state.current_line_len += s.len();
        }
        FmtOp::QuotedText(s) => {
            state.output.push(b'"');
            state.output.extend_from_slice(s.as_bytes());
            state.output.push(b'"');
            state.current_line_len += s.len() + 2;
        }
        FmtOp::Char(c) => {
            state.output.push(*c);
            state.current_line_len += 1;
        }
        FmtOp::I64(v) => emit_int(*v, state),
        FmtOp::U64(v) => emit_int(*v, state),
        FmtOp::F64(v) => emit_f64(*v, state),
        FmtOp::Hardline => emit_newline(state, indent_cache),
        FmtOp::Softline => {
            if state.in_break_mode() {
                emit_newline(state, indent_cache);
            } else {
                state.output.push(b' ');
                state.current_line_len += 1;
            }
        }
        FmtOp::BreakLine => {
            if state.in_break_mode() {
                emit_newline(state, indent_cache);
            }
        }
        FmtOp::Mediumline => {
            if state.current_line_len > state.max_width / 2 {
                emit_newline(state, indent_cache);
            }
        }
        FmtOp::CommaSep => {
            if state.in_break_mode() {
                state.output.push(b',');
                state.current_line_len += 1;
                emit_newline(state, indent_cache);
            } else {
                state.output.extend_from_slice(b", ");
                state.current_line_len += 2;
            }
        }
        FmtOp::Sep {
            flat,
            flat_len,
            brk,
            brk_len,
        } => {
            if state.in_break_mode() {
                state
                    .output
                    .extend_from_slice(&brk[..*brk_len as usize]);
                state.current_line_len += *brk_len as usize;
                emit_newline(state, indent_cache);
            } else {
                state
                    .output
                    .extend_from_slice(&flat[..*flat_len as usize]);
                state.current_line_len += *flat_len as usize;
            }
        }
        FmtOp::GroupStart { flat_width } => {
            let breaks = state.current_line_len + *flat_width as usize > state.max_width;
            state.push_group(breaks);
        }
        FmtOp::GroupEnd => {
            state.pop_group();
        }
        FmtOp::IndentStart => {
            state.indent += state.indent_size;
        }
        FmtOp::IndentEnd => {
            state.indent = state.indent.saturating_sub(state.indent_size);
        }
        FmtOp::WrapStart { open, flat_width } => {
            let breaks = state.current_line_len + *flat_width as usize > state.max_width;
            state.push_group(breaks);
            state.output.push(*open);
            state.current_line_len += 1;
            state.indent += state.indent_size;
            if breaks {
                emit_newline(state, indent_cache);
            }
        }
        FmtOp::WrapEnd { close } => {
            state.indent = state.indent.saturating_sub(state.indent_size);
            if state.in_break_mode() {
                emit_newline(state, indent_cache);
            }
            state.output.push(*close);
            state.current_line_len += 1;
            state.pop_group();
        }
        // Wide-ops and IfBreak shouldn't appear inside IfBreak sub-ranges.
        FmtOp::I128Hi(_)
        | FmtOp::I128Lo(_)
        | FmtOp::U128Hi(_)
        | FmtOp::U128Lo(_)
        | FmtOp::IfBreak { .. } => {}
    }
}
