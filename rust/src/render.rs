use crate::op::FmtOp;

/// Linear-scan renderer. Single pass over the op buffer, O(n) time,
/// zero allocations beyond the output buffer.
///
/// Break decisions are O(1) per Group — `flat_width` is pre-computed
/// by the `FmtBuilder` during construction.
pub fn render(ops: &[FmtOp<'_>], max_width: usize, indent_size: usize) -> String {
    render_with_hint(ops, max_width, indent_size, 0)
}

/// Like `render`, but with a size hint for the output buffer.
pub fn render_with_hint(
    ops: &[FmtOp<'_>],
    max_width: usize,
    indent_size: usize,
    size_hint: usize,
) -> String {
    let mut state = RenderState {
        output: Vec::with_capacity(if size_hint > 0 { size_hint } else { ops.len() * 4 }),
        group_stack: Vec::with_capacity(16),
        indent: 0,
        current_line_len: 0,
        max_width,
        indent_size,
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

            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                if state.in_break_mode() {
                    // Emit next break_count ops, then skip flat_count.
                    // The break ops are at pc+1..pc+1+break_count.
                    // The flat ops are at pc+1+break_count..pc+1+break_count+flat_count.
                    // After processing, pc advances past all of them.
                    let break_end = pc + 1 + break_count as usize;
                    for i in (pc + 1)..break_end {
                        render_single_op(&ops[i], &mut state);
                    }
                    pc = break_end + flat_count as usize;
                    continue;
                } else {
                    // Skip break_count ops, emit flat_count.
                    let flat_start = pc + 1 + break_count as usize;
                    let flat_end = flat_start + flat_count as usize;
                    for i in flat_start..flat_end {
                        render_single_op(&ops[i], &mut state);
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

    // SAFETY: all ops emit valid UTF-8 (Text borrows &str, Char emits ASCII).
    unsafe { String::from_utf8_unchecked(state.output) }
}

struct RenderState {
    output: Vec<u8>,
    group_stack: Vec<bool>, // true = break mode
    indent: usize,
    current_line_len: usize,
    max_width: usize,
    indent_size: usize,
}

impl RenderState {
    #[inline(always)]
    fn in_break_mode(&self) -> bool {
        self.group_stack.last().copied().unwrap_or(false)
    }
}

#[inline(always)]
fn emit_newline(state: &mut RenderState) {
    // Strip trailing whitespace before the break.
    while state.output.last().is_some_and(|&b| b == b' ' || b == b'\t') {
        state.output.pop();
    }
    state.output.push(b'\n');
    // Emit indent.
    for _ in 0..state.indent {
        state.output.push(b' ');
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
            // Silently skip.
        }
    }
}
