//! The compile step of `FmtBuilder::finish()` (sk-v27 LW2.PP-2).
//!
//! Two passes over the op stream, each op visited once per pass:
//!
//! 1. **Forward** — validate the nesting (law L8: an unbalanced stream is an
//!    `Err`, never a guess), number the group headers, record each group's
//!    `end_pc`, and propagate `HAS_HARD` (law L2) from a hard line to its
//!    innermost group and from a closing group to its parent.
//! 2. **Backward** — compute each group's `trailing` (law L3′).
//!
//! ## `trailing`, from [W] §4
//!
//! Wadler's `better w k x y = if fits (w-k) x then x else y` asks `fits` about
//! the flattened group *followed by the rest of the document*, up to the first
//! `Line` the rest yields. Any later group in that rest is itself a union, and
//! `be` resolves it with its own `better` while `fits` is reading. Call
//! `F(p)` the width the stream from op `p` yields before its first newline,
//! under that resolution. For a later group `H` at `p`, with `r` columns left:
//!
//! - if `H`'s flat side fits (`r >= flat(H) + F(after H)`), `be` takes it and
//!   `fits` reads on through `H` and its own rest, which fits by the same
//!   inequality;
//! - otherwise `be` takes `H`'s broken side, and `fits` reads `H`'s broken
//!   content up to its first newline, `F(p + 1)`.
//!
//! So the stream from `H` fits in `r` iff `r >= min(flat(H) + F(after H),
//! F(p + 1))`, which does not depend on the column. That gives one backward
//! recurrence, O(1) per op:
//!
//! ```text
//! F(end)            = 0                               -- fits w Nil = True
//! F(leaf)           = width(leaf) + F(next)            -- fits (w - length s) x
//! F(line op)        = width before its newline         -- fits w (i `Line` x) = True
//! F(GroupStart H)   = min(flat(H) + F(after H), F(first child))
//!                     (flat side = infinity when H is HAS_HARD: fits _ SFail = False)
//! F(WrapStart H)    = min(flat(H) + F(after H), 1)     -- broken: open, then newline
//! F(IfBreak)        = F of the branch the context takes
//! trailing(G)       = F(after G)
//! ```
//!
//! and the renderer's decision `col + flat(G) + trailing(G) <= w` is exactly
//! `fits (w - col) (flatten G ++ rest)`. At depth 0 pprint's convention holds
//! (lines outside every group are flat, as in the oracle's lowering), so a
//! line op there counts its flat width and the recurrence reads on.
//!
//! "After" follows control flow, not the array: the last op of an `IfBreak`'s
//! break branch is followed by the op after the whole `IfBreak` (`chase`).

use crate::DigitCount;
use crate::op::{Doc, FmtOp, GroupHdr};

/// A stream the builder refuses (law L8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// The group, indent and `IfBreak` regions are not well nested.
    Unbalanced { pc: u32, reason: &'static str },
    /// The stream has more than `u32::MAX - 1` ops (ABI v2 indexes are u32).
    TooLong,
    /// A byte the op stream carries as an [`AsciiByte`](crate::AsciiByte)
    /// (`char`, `wrap`'s delimiters, `sep`'s bytes) is not printable ASCII:
    /// a non-ASCII byte, a tab, a line feed or another control byte
    /// (LW3.PP-1a).
    NotAscii { pc: u32, byte: u8 },
    /// A text leaf holds a tab, whose display width depends on tab stops the
    /// renderer does not have (LW3.PP-1a).
    Tab { pc: u32 },
    /// A `sep` longer than its op holds (8 flat bytes, 4 break bytes).
    SepTooLong { pc: u32 },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Unbalanced { pc, reason } => {
                write!(f, "unbalanced op stream at op {pc}: {reason}")
            }
            BuildError::TooLong => write!(f, "op stream longer than u32::MAX - 1 ops"),
            BuildError::NotAscii { pc, byte } => write!(
                f,
                "op {pc}: byte {byte:#04x} is not printable ASCII (an AsciiByte is 0x20..=0x7E)"
            ),
            BuildError::Tab { pc } => write!(f, "op {pc}: a tab in a text leaf"),
            BuildError::SepTooLong { pc } => write!(
                f,
                "op {pc}: a sep longer than 8 flat bytes or 4 break bytes"
            ),
        }
    }
}

impl std::error::Error for BuildError {}

/// Larger than any real width, small enough that sums never overflow u64.
const INF: u64 = u64::MAX / 4;

enum Frame {
    Group,
    Wrap,
    Indent,
    /// An `IfBreak` branch ending at `end`; `brk` for the break branch.
    Region {
        end: u32,
        brk: bool,
    },
}

/// The display width of one leaf op (byte length; display columns are
/// LW3.PP-1's `width.rs`). Line and structural ops are 0 here.
#[inline]
pub(crate) fn leaf_width(ops: &[FmtOp<'_>], p: usize) -> u64 {
    match &ops[p] {
        FmtOp::Text(s) => s.len() as u64,
        FmtOp::OwnedText(s) => s.len() as u64,
        FmtOp::QuotedText(s) => s.len() as u64 + 2,
        FmtOp::Char(_) => 1,
        FmtOp::I64(v) => v.len() as u64,
        FmtOp::U64(v) => v.len() as u64,
        FmtOp::I128Hi(hi) => match ops.get(p + 1) {
            Some(FmtOp::I128Lo(lo)) => ((((*hi as i128) << 64) | (*lo as i128)).len()) as u64,
            _ => 0,
        },
        FmtOp::U128Hi(hi) => match ops.get(p + 1) {
            Some(FmtOp::U128Lo(lo)) => ((((*hi as u128) << 64) | (*lo as u128)).len()) as u64,
            _ => 0,
        },
        FmtOp::F64(v) => f64_width(*v) as u64,
        _ => 0,
    }
}

/// The exact width the renderer prints for an `F64` (ryu, as `render.rs`).
#[inline]
pub(crate) fn f64_width(v: f64) -> usize {
    ryu::Buffer::new().format_finite(v).len()
}

fn unbalanced<T>(pc: usize, reason: &'static str) -> Result<T, BuildError> {
    Err(BuildError::Unbalanced {
        pc: pc as u32,
        reason,
    })
}

/// Validate `ops`, build the header table and compute `trailing`.
///
/// On entry each `GroupStart.hdr` / `WrapStart.hdr` holds the group's flat
/// width (the builder's running width); on exit it holds the header index.
pub(crate) fn compile(mut ops: Vec<FmtOp<'_>>) -> Result<Doc<'_>, BuildError> {
    let n = ops.len();
    if n >= u32::MAX as usize {
        return Err(BuildError::TooLong);
    }

    let mut hdrs: Vec<GroupHdr> = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    // Open groups as (header index, break-region depth at open).
    let mut open: Vec<(u32, u32)> = Vec::new();
    // Ends of the open `IfBreak` regions, innermost last.
    let mut region_ends: Vec<u32> = Vec::new();
    let mut brk_depth: u32 = 0;
    let mut depth: u32 = 0;
    // `root[p]`: op p lies outside every group (pprint's flat context).
    let mut root: Vec<bool> = Vec::with_capacity(n);
    // `jump[x]`: falling off a non-empty break branch that ends at x continues
    // at the op after its `IfBreak` (the outermost such, when several end at x).
    let mut jump: Vec<u32> = (0..=n as u32).collect();

    // ── Pass 1: forward ─────────────────────────────────────────────────
    let mut pc = 0usize;
    while pc <= n {
        // Close every region that ends here.
        while let Some(&end) = region_ends.last() {
            if end as usize > pc {
                break;
            }
            match frames.pop() {
                Some(Frame::Region { end: e, brk }) if e as usize == pc => {
                    region_ends.pop();
                    if brk {
                        brk_depth -= 1;
                    }
                }
                _ => return unbalanced(pc, "a group or indent crosses an IfBreak branch end"),
            }
        }
        if pc == n {
            break;
        }
        root.push(depth == 0);
        match ops[pc] {
            FmtOp::GroupStart { hdr: w } | FmtOp::WrapStart { hdr: w, .. } => {
                let is_wrap = matches!(ops[pc], FmtOp::WrapStart { .. });
                let idx = hdrs.len() as u32;
                hdrs.push(GroupHdr {
                    flat_width: w,
                    trailing: 0,
                    end_pc: 0,
                    flags: if is_wrap { GroupHdr::WRAP } else { 0 },
                });
                match &mut ops[pc] {
                    FmtOp::GroupStart { hdr } | FmtOp::WrapStart { hdr, .. } => *hdr = idx,
                    _ => unreachable!(),
                }
                frames.push(if is_wrap { Frame::Wrap } else { Frame::Group });
                open.push((idx, brk_depth));
                depth += 1;
            }
            FmtOp::GroupEnd | FmtOp::WrapEnd { .. } => {
                let is_wrap = matches!(ops[pc], FmtOp::WrapEnd { .. });
                match (frames.pop(), is_wrap) {
                    (Some(Frame::Group), false) | (Some(Frame::Wrap), true) => {}
                    _ => return unbalanced(pc, "a group end with no matching group start"),
                }
                let (idx, d) = open.pop().expect("frame and open stacks agree");
                hdrs[idx as usize].end_pc = pc as u32;
                // L2: a hard line in the child's flattening is one in the
                // parent's, unless an IfBreak break branch separates them
                // (`flatten (flatAlt x y) = flatten y`).
                if hdrs[idx as usize].has_hard()
                    && let Some(&(parent, pd)) = open.last()
                    && pd == d
                {
                    hdrs[parent as usize].flags |= GroupHdr::HAS_HARD;
                }
                depth -= 1;
            }
            FmtOp::IndentStart => frames.push(Frame::Indent),
            FmtOp::IndentEnd => match frames.pop() {
                Some(Frame::Indent) => {}
                _ => return unbalanced(pc, "IndentEnd with no matching IndentStart"),
            },
            FmtOp::Hardline => {
                if let Some(&(g, d)) = open.last()
                    && d == brk_depth
                {
                    hdrs[g as usize].flags |= GroupHdr::HAS_HARD;
                }
            }
            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                let flat_start = pc as u64 + 1 + break_count as u64;
                let rest = flat_start + flat_count as u64;
                let limit = region_ends.last().map_or(n as u64, |&e| e as u64);
                if rest > limit {
                    return unbalanced(pc, "IfBreak branches run past their enclosing region");
                }
                let (flat_start, rest) = (flat_start as u32, rest as u32);
                frames.push(Frame::Region {
                    end: rest,
                    brk: false,
                });
                region_ends.push(rest);
                frames.push(Frame::Region {
                    end: flat_start,
                    brk: true,
                });
                region_ends.push(flat_start);
                brk_depth += 1;
                if break_count > 0 {
                    let j = &mut jump[flat_start as usize];
                    *j = (*j).max(rest);
                }
            }
            FmtOp::I128Hi(_) | FmtOp::U128Hi(_) => {
                let paired = matches!(
                    (&ops[pc], ops.get(pc + 1)),
                    (FmtOp::I128Hi(_), Some(FmtOp::I128Lo(_)))
                        | (FmtOp::U128Hi(_), Some(FmtOp::U128Lo(_)))
                );
                if !paired {
                    return unbalanced(pc, "a 128-bit high op without its low op");
                }
                root.push(depth == 0);
                pc += 1;
            }
            FmtOp::I128Lo(_) | FmtOp::U128Lo(_) => {
                return unbalanced(pc, "a 128-bit low op without its high op");
            }
            _ => {}
        }
        pc += 1;
    }
    if !frames.is_empty() {
        return unbalanced(n, "an unclosed group or indent at the end");
    }

    // ── Pass 2: backward ────────────────────────────────────────────────
    let chase = |mut x: usize| -> usize {
        loop {
            let j = jump[x] as usize;
            if j == x {
                return x;
            }
            x = j;
        }
    };
    let mut f: Vec<u64> = vec![0; n + 1];
    for p in (0..n).rev() {
        let at_root = root[p];
        let next = || chase(p + 1);
        f[p] = match &ops[p] {
            FmtOp::Text(_)
            | FmtOp::OwnedText(_)
            | FmtOp::QuotedText(_)
            | FmtOp::Char(_)
            | FmtOp::I64(_)
            | FmtOp::U64(_)
            | FmtOp::F64(_)
            | FmtOp::I128Hi(_)
            | FmtOp::U128Hi(_) => leaf_width(&ops, p) + f[next()],
            FmtOp::I128Lo(_) | FmtOp::U128Lo(_) | FmtOp::IndentStart | FmtOp::IndentEnd => {
                f[next()]
            }
            FmtOp::Hardline => 0,
            FmtOp::Softline => {
                if at_root {
                    1 + f[next()]
                } else {
                    0
                }
            }
            FmtOp::BreakLine => {
                if at_root {
                    f[next()]
                } else {
                    0
                }
            }
            FmtOp::CommaSep => {
                if at_root {
                    2 + f[next()]
                } else {
                    1
                }
            }
            FmtOp::Sep {
                flat_len, brk_len, ..
            } => {
                if at_root {
                    *flat_len as u64 + f[next()]
                } else {
                    *brk_len as u64
                }
            }
            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                let flat_start = p + 1 + *break_count as usize;
                let rest = flat_start + *flat_count as usize;
                let (start, len) = if at_root {
                    (flat_start, *flat_count)
                } else {
                    (p + 1, *break_count)
                };
                if len > 0 { f[start] } else { f[chase(rest)] }
            }
            FmtOp::GroupStart { hdr } | FmtOp::WrapStart { hdr, .. } => {
                let h = &mut hdrs[*hdr as usize];
                let after = f[chase(h.end_pc as usize + 1)];
                h.trailing = after.min(u32::MAX as u64) as u32;
                let flat_side = if h.has_hard() {
                    INF
                } else {
                    h.flat_width as u64 + after
                };
                let broken_side = if h.flags & GroupHdr::WRAP != 0 {
                    1
                } else {
                    f[p + 1]
                };
                flat_side.min(broken_side)
            }
            FmtOp::GroupEnd => f[next()],
            FmtOp::WrapEnd { .. } => 0,
        };
    }

    Ok(Doc { ops, hdrs })
}
