//! Lowering of the `FmtOp` stream (ABI v2 since LW2.PP-1; the group headers
//! are not read: the oracle decides every group itself) onto the oracle's tree.
//!
//! This reads `FmtOp` through its public enum only; it never calls into
//! `render.rs`. Each op's lowering cites the oracle construct it denotes.
//!
//! One convention is pprint's, not the papers': outside every group the
//! v1 renderer is in *flat* mode (`in_break_mode()` is false at depth 0), so a
//! line op at depth 0 lowers to its flattened form, while a `Hardline` stays a
//! newline. Inside a group, the group's own `[W] §3 group` union decides.

use super::*;
use pprint::{AsciiByte, FmtOp, Printer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// The stream is not a well-nested tree of Group/Indent/Wrap regions
    /// (law L8: unbalanced input is an `Err`, never a guess).
    Unbalanced(&'static str),
    /// A wide op without its partner.
    WideOp,
}

enum Frame {
    Root,
    Group,
    Indent,
    Wrap { open: AsciiByte, close_seen: bool },
}

struct Level {
    frame: Frame,
    parts: Vec<D>,
}

fn seq(parts: Vec<D>) -> D {
    parts.into_iter().rev().fold(nil(), |acc, d| match &*acc {
        Doc::Nil => d,
        _ => cat(d, acc),
    })
}

fn byte_text(bytes: &[AsciiByte]) -> D {
    // Sep / Wrap / Char bytes are `AsciiByte`s (LW3.PP-1a): printable ASCII,
    // one `Char` each in [W]'s `length`.
    text(&bytes.iter().map(|b| b.as_char()).collect::<String>())
}

/// Lower a v1 op stream to an oracle document under `printer`.
pub fn lower(ops: &[FmtOp<'_>], printer: Printer) -> Result<D, LowerError> {
    lower_range(ops, printer, 0)
}

fn lower_range(ops: &[FmtOp<'_>], printer: Printer, base_depth: usize) -> Result<D, LowerError> {
    let ind = printer.indent as isize;
    let mut stack: Vec<Level> = vec![Level {
        frame: Frame::Root,
        parts: Vec::new(),
    }];
    let mut group_depth = base_depth;
    let mut pc = 0;
    while pc < ops.len() {
        let flat_ctx = group_depth == 0;
        let leaf: Option<D> = match &ops[pc] {
            // A leaf's line feeds are hard lines at column 0 (LW3.PP-1b,
            // `leaf_text`): the last line's width, and `HAS_HARD`.
            FmtOp::Text(s) => Some(leaf_text(s)),
            FmtOp::OwnedText(s) => Some(leaf_text(s)),
            FmtOp::QuotedText(s) => Some(leaf_text(&format!("\"{s}\""))),
            FmtOp::Char(c) => Some(byte_text(&[*c])),
            FmtOp::I64(v) => Some(text(&v.to_string())),
            FmtOp::U64(v) => Some(text(&v.to_string())),
            FmtOp::I128Hi(hi) => match ops.get(pc + 1) {
                Some(FmtOp::I128Lo(lo)) => {
                    pc += 1;
                    Some(text(&(((*hi as i128) << 64) | (*lo as i128)).to_string()))
                }
                _ => return Err(LowerError::WideOp),
            },
            FmtOp::U128Hi(hi) => match ops.get(pc + 1) {
                Some(FmtOp::U128Lo(lo)) => {
                    pc += 1;
                    Some(text(&(((*hi as u128) << 64) | (*lo as u128)).to_string()))
                }
                _ => return Err(LowerError::WideOp),
            },
            FmtOp::I128Lo(_) | FmtOp::U128Lo(_) => return Err(LowerError::WideOp),
            FmtOp::F64(v) => Some(text(ryu::Buffer::new().format_finite(*v))),

            // [W] §3 `line`; at depth 0, `flatten line = text " "`.
            FmtOp::Softline => Some(if flat_ctx { text(" ") } else { line() }),
            // [L] `linebreak`; at depth 0, `flatten linebreak = empty`.
            FmtOp::BreakLine => Some(if flat_ctx { nil() } else { linebreak() }),
            // [P] `hardline`.
            FmtOp::Hardline => Some(hardline()),
            // `text "," <> line`: [W] `flatten` gives ", ".
            FmtOp::CommaSep => Some(if flat_ctx {
                text(", ")
            } else {
                cat(text(","), line())
            }),
            // [P] `flatAlt (text brk <> line) (text flat)`.
            FmtOp::Sep {
                flat,
                flat_len,
                brk,
                brk_len,
            } => {
                let f = byte_text(&flat[..*flat_len as usize]);
                let b = cat(byte_text(&brk[..*brk_len as usize]), line());
                Some(if flat_ctx { f } else { flat_alt(b, f) })
            }
            // [P] `flatAlt break_branch flat_branch`, branch ranges lowered
            // recursively in the same context.
            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                let b0 = pc + 1;
                let b1 = b0 + *break_count as usize;
                let f1 = b1 + *flat_count as usize;
                if f1 > ops.len() {
                    return Err(LowerError::Unbalanced("IfBreak range past end"));
                }
                let fb = lower_range(&ops[b1..f1], printer, group_depth)?;
                let d = if flat_ctx {
                    fb
                } else {
                    flat_alt(lower_range(&ops[b0..b1], printer, group_depth)?, fb)
                };
                pc = f1;
                stack.last_mut().unwrap().parts.push(d);
                continue;
            }

            FmtOp::GroupStart { .. } => {
                group_depth += 1;
                stack.push(Level {
                    frame: Frame::Group,
                    parts: Vec::new(),
                });
                None
            }
            FmtOp::IndentStart => {
                stack.push(Level {
                    frame: Frame::Indent,
                    parts: Vec::new(),
                });
                None
            }
            FmtOp::WrapStart { open, .. } => {
                group_depth += 1;
                stack.push(Level {
                    frame: Frame::Wrap {
                        open: *open,
                        close_seen: false,
                    },
                    parts: Vec::new(),
                });
                None
            }
            FmtOp::GroupEnd => {
                let lv = stack.pop().unwrap();
                if !matches!(lv.frame, Frame::Group) {
                    return Err(LowerError::Unbalanced("GroupEnd closes a non-group"));
                }
                group_depth -= 1;
                // [W] §3 `group x = flatten x :<|> x`.
                Some(group(seq(lv.parts)))
            }
            FmtOp::IndentEnd => {
                let lv = stack.pop().unwrap();
                if !matches!(lv.frame, Frame::Indent) {
                    return Err(LowerError::Unbalanced("IndentEnd closes a non-indent"));
                }
                // [W] §3 `nest i x`.
                Some(nest(ind, seq(lv.parts)))
            }
            FmtOp::WrapEnd { close } => {
                let lv = stack.pop().unwrap();
                let open = match lv.frame {
                    Frame::Wrap { open, close_seen } => {
                        debug_assert!(!close_seen);
                        open
                    }
                    _ => return Err(LowerError::Unbalanced("WrapEnd closes a non-wrap")),
                };
                group_depth -= 1;
                // `group (open <> nest i (linebreak <> body) <> linebreak <> close)`.
                Some(group(seq(vec![
                    byte_text(&[open]),
                    nest(ind, cat(linebreak(), seq(lv.parts))),
                    linebreak(),
                    byte_text(&[*close]),
                ])))
            }
        };
        if let Some(d) = leaf {
            // The Root frame is never popped (closing it is an `Unbalanced`
            // error above), so the stack is non-empty here.
            stack.last_mut().unwrap().parts.push(d);
        }
        pc += 1;
    }
    if stack.len() != 1 {
        return Err(LowerError::Unbalanced("unclosed region at end"));
    }
    Ok(seq(stack.pop().unwrap().parts))
}

/// Render a v1 stream through the oracle: lower, then [W] §4 `pretty` with
/// pprint's trailing-whitespace convention.
pub fn oracle_render(ops: &[FmtOp<'_>], printer: Printer) -> Result<String, LowerError> {
    let d = lower(ops, printer)?;
    Ok(
        pretty_pprint(printer.max_width as isize, &d)
            .expect("lowered v1 streams never force SFail"),
    )
}
