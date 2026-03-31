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
    let mut state = RenderState {
        output: Vec::with_capacity(if size_hint > 0 { size_hint } else { ops.len() * 4 }),
        group_stack: Vec::with_capacity(16),
        indent: 0,
        current_line_len: 0,
        max_width: printer.max_width,
        indent_size: printer.indent,
        use_tabs: printer.use_tabs,
    };

    let mut pc = 0;
    while pc < ops.len() {
        match ops[pc] {
            FmtOp::Text(s) => {
                state.output.extend_from_slice(s.as_bytes());
                state.current_line_len += s.len();
            }

            FmtOp::Char(c) => {
                state.output.push(c);
                state.current_line_len += 1;
            }

            FmtOp::I64(v) => emit_int(v, &mut state),
            FmtOp::U64(v) => emit_int(v, &mut state),
            FmtOp::I128(v) => emit_int(v, &mut state),
            FmtOp::U128(v) => emit_int(v, &mut state),
            FmtOp::F64(v) => emit_f64(v, &mut state),

            FmtOp::GroupStart { flat_width } => {
                let breaks = state.current_line_len + flat_width as usize > state.max_width;
                state.group_stack.push(breaks);
            }

            FmtOp::GroupEnd => {
                state.group_stack.pop();
            }

            FmtOp::IndentStart => {
                state.indent += state.indent_size;
            }

            FmtOp::IndentEnd => {
                state.indent = state.indent.saturating_sub(state.indent_size);
            }

            FmtOp::Hardline => {
                emit_newline(&mut state);
            }

            FmtOp::Softline => {
                if state.in_break_mode() {
                    emit_newline(&mut state);
                } else {
                    state.output.push(b' ');
                    state.current_line_len += 1;
                }
            }

            FmtOp::BreakLine => {
                if state.in_break_mode() {
                    emit_newline(&mut state);
                }
                // In flat mode: emit nothing.
            }

            FmtOp::Mediumline => {
                if state.current_line_len > state.max_width / 2 {
                    emit_newline(&mut state);
                }
                // Under half-width: emit nothing.
            }

            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                if state.in_break_mode() {
                    let break_end = pc + 1 + break_count as usize;
                    for op in &ops[pc + 1..break_end] {
                        render_single_op(op, &mut state);
                    }
                    pc = break_end + flat_count as usize;
                    continue;
                } else {
                    let flat_start = pc + 1 + break_count as usize;
                    let flat_end = flat_start + flat_count as usize;
                    for op in &ops[flat_start..flat_end] {
                        render_single_op(op, &mut state);
                    }
                    pc = flat_end;
                    continue;
                }
            }
        }

        pc += 1;
    }

    // Strip trailing whitespace.
    while state.output.last().is_some_and(|&b| b == b' ' || b == b'\t') {
        state.output.pop();
    }

    // SAFETY: all ops emit valid UTF-8 (Text borrows &str, Char emits ASCII,
    // numeric ops produce ASCII digits).
    unsafe { String::from_utf8_unchecked(state.output) }
}

struct RenderState {
    output: Vec<u8>,
    group_stack: Vec<bool>, // true = break mode
    indent: usize,
    current_line_len: usize,
    max_width: usize,
    indent_size: usize,
    use_tabs: bool,
}

impl RenderState {
    #[inline(always)]
    fn in_break_mode(&self) -> bool {
        self.group_stack.last().copied().unwrap_or(false)
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
    let mut buf = dragonbox::Buffer::new();
    let s = buf.format_finite(value).as_bytes();
    state.output.extend_from_slice(s);
    state.current_line_len += s.len();
}

#[inline(always)]
fn emit_newline(state: &mut RenderState) {
    // Strip trailing whitespace before the break.
    while state.output.last().is_some_and(|&b| b == b' ' || b == b'\t') {
        state.output.pop();
    }
    state.output.push(b'\n');
    // Emit indent.
    let ch = if state.use_tabs { b'\t' } else { b' ' };
    for _ in 0..state.indent {
        state.output.push(ch);
    }
    state.current_line_len = state.indent;
}

/// Render a single op (used by IfBreak to process sub-ranges).
#[inline(always)]
fn render_single_op(op: &FmtOp<'_>, state: &mut RenderState) {
    match op {
        FmtOp::Text(s) => {
            state.output.extend_from_slice(s.as_bytes());
            state.current_line_len += s.len();
        }
        FmtOp::Char(c) => {
            state.output.push(*c);
            state.current_line_len += 1;
        }
        FmtOp::I64(v) => emit_int(*v, state),
        FmtOp::U64(v) => emit_int(*v, state),
        FmtOp::I128(v) => emit_int(*v, state),
        FmtOp::U128(v) => emit_int(*v, state),
        FmtOp::F64(v) => emit_f64(*v, state),
        FmtOp::Hardline => emit_newline(state),
        FmtOp::Softline => {
            if state.in_break_mode() {
                emit_newline(state);
            } else {
                state.output.push(b' ');
                state.current_line_len += 1;
            }
        }
        FmtOp::BreakLine => {
            if state.in_break_mode() {
                emit_newline(state);
            }
        }
        FmtOp::Mediumline => {
            if state.current_line_len > state.max_width / 2 {
                emit_newline(state);
            }
        }
        FmtOp::GroupStart { flat_width } => {
            let breaks = state.current_line_len + *flat_width as usize > state.max_width;
            state.group_stack.push(breaks);
        }
        FmtOp::GroupEnd => {
            state.group_stack.pop();
        }
        FmtOp::IndentStart => {
            state.indent += state.indent_size;
        }
        FmtOp::IndentEnd => {
            state.indent = state.indent.saturating_sub(state.indent_size);
        }
        FmtOp::IfBreak { .. } => {
            // Nested IfBreak inside IfBreak — shouldn't happen in well-formed ops.
        }
    }
}
