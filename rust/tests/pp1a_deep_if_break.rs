//! sk-v27 LW3.PP-1a: a 10⁶-deep `IfBreak` nest renders at the default stack.
//!
//! At entry the renderer recursed once per `IfBreak` it entered, so this
//! nest overflowed the test thread's default stack and aborted the binary.
//! The one-loop renderer keeps the open branches on an explicit heap stack.
//!
//! The document is built from the public `Doc` fields, since the builder's
//! closure form would itself recurse 10⁶ deep in the caller.

use pprint::{Doc, FmtOp, GroupHdr, PRINTER, render};

const N: usize = 1_000_000;

/// `[GroupStart, IfBreak × N, Text("x"), GroupEnd]`: every `IfBreak`'s break
/// branch holds the rest of the nest and its flat branch is empty, inside a
/// `HAS_HARD` group, so every break branch is taken.
#[test]
fn if_break_nest_10_6_in_break_mode() {
    let mut ops = Vec::with_capacity(N + 3);
    ops.push(FmtOp::GroupStart { hdr: 0 });
    for k in 0..N {
        ops.push(FmtOp::IfBreak {
            break_count: (N - k) as u32,
            flat_count: 0,
        });
    }
    ops.push(FmtOp::Text("x"));
    ops.push(FmtOp::GroupEnd);
    let hdrs = vec![GroupHdr {
        flat_width: 0,
        trailing: 0,
        trailing_pending: 0,
        cut: GroupHdr::NO_CUT,
        cut_pending: GroupHdr::NO_CUT,
        end_pc: (N + 2) as u32,
        flags: GroupHdr::HAS_HARD,
    }];
    let doc = Doc { ops, hdrs };
    assert_eq!(render(&doc, PRINTER), "x");
}

/// The same nest through the flat branches, at depth 0 (pprint's flat
/// convention): every `IfBreak` takes its flat branch, which holds the rest.
#[test]
fn if_break_nest_10_6_in_flat_mode() {
    let mut ops = Vec::with_capacity(N + 1);
    for k in 0..N {
        ops.push(FmtOp::IfBreak {
            break_count: 0,
            flat_count: (N - k) as u32,
        });
    }
    ops.push(FmtOp::Text("y"));
    let doc = Doc {
        ops,
        hdrs: Vec::new(),
    };
    assert_eq!(render(&doc, PRINTER), "y");
}
