//! sk-v27 LW3.PP-2: what a `LineSuffix` may hold (leaves only; L8 for the
//! new op) and the ops the trivia builders emit.

use pprint::{BuildError, FmtBuilder, FmtOp};

/// `finish()` refuses a suffix that holds anything but leaves.
#[test]
fn a_suffix_holds_leaves_only() {
    let mut b = FmtBuilder::new();
    b.line_suffix(|b| {
        b.text("x");
        b.hardline();
    });
    assert!(b.finish().is_err());
    let mut b = FmtBuilder::new();
    b.line_suffix(|b| b.group(|b| b.text("x")));
    assert!(b.finish().is_err());
    let mut b = FmtBuilder::new();
    b.line_suffix(|b| {
        b.text("x");
        b.i128(-1);
        b.char(b';');
    });
    let doc = b.finish().unwrap();
    assert!(matches!(doc.ops()[0], FmtOp::LineSuffix { len: 4 }));
}

/// An empty suffix emits no op; a boundary and a break-parent are one op
/// each.
#[test]
fn the_trivia_builders_emit_one_op_each() {
    let mut b = FmtBuilder::new();
    b.line_suffix(|_| {});
    b.line_suffix_boundary();
    b.break_parent();
    let doc = b.finish().unwrap();
    assert!(matches!(
        doc.ops(),
        [FmtOp::LineSuffixBoundary, FmtOp::BreakParent]
    ));
}

/// The refusal names the op that is not a leaf.
#[test]
fn a_non_leaf_in_a_suffix_is_suffix_content() {
    let mut b = FmtBuilder::new();
    b.text("a");
    b.line_suffix(|b| b.softline());
    assert_eq!(b.finish().unwrap_err(), BuildError::SuffixContent { pc: 2 });
}
