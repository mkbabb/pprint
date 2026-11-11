//! sk-v27 LW2.PP-2, law L8: an unbalanced stream is an `Err` from
//! `finish()`, never a guess and never a panic.
//!
//! The test body reads `finish()` through `Refused`, which is implemented for
//! both the v1 return type (`Vec<FmtOp>`, which can never refuse) and the v2
//! one (`Result<Doc, BuildError>`), so the same file reads RED at the v1 base
//! (`finish()` silently closes what was left open, or `group_close` panics)
//! and GREEN once the builder refuses.

use pprint::FmtBuilder;
use std::panic::{AssertUnwindSafe, catch_unwind};

trait Refused {
    fn refused(&self) -> bool;
}

impl<T> Refused for Vec<T> {
    fn refused(&self) -> bool {
        false
    }
}

impl<T, E> Refused for Result<T, E> {
    fn refused(&self) -> bool {
        self.is_err()
    }
}

/// Run `f` on a fresh builder; true iff `finish()` refused without panicking.
fn refuses(f: impl FnOnce(&mut FmtBuilder<'static>)) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let mut b = FmtBuilder::new();
        f(&mut b);
        b.finish().refused()
    }))
    .unwrap_or(false)
}

/// Run `f` on a fresh builder; true iff `finish()` accepted without panicking.
fn accepts(f: impl FnOnce(&mut FmtBuilder<'static>)) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let mut b = FmtBuilder::new();
        f(&mut b);
        !b.finish().refused()
    }))
    .unwrap_or(false)
}

#[test]
fn l8_unclosed_group_is_refused() {
    assert!(
        refuses(|b| {
            b.group_open();
            b.text("a");
        }),
        "finish() must refuse an unclosed group_open, not close it"
    );
}

#[test]
fn l8_close_without_open_is_refused() {
    assert!(
        refuses(|b| {
            b.text("a");
            b.group_close();
        }),
        "group_close with nothing open must make finish() refuse, not panic"
    );
}

#[test]
fn l8_unclosed_indent_is_refused() {
    assert!(
        refuses(|b| {
            b.indent_open();
            b.text("a");
        }),
        "finish() must refuse an unclosed indent_open"
    );
}

#[test]
fn l8_indent_close_without_open_is_refused() {
    assert!(
        refuses(|b| {
            b.text("a");
            b.indent_close();
        }),
        "finish() must refuse an indent_close with nothing open"
    );
}

#[test]
fn l8_group_crossing_an_if_break_branch_is_refused() {
    assert!(
        refuses(|b| {
            b.group(|b| {
                b.if_break(|b| b.group_open(), |b| b.group_close());
            });
        }),
        "a group opened in one IfBreak branch and closed in the other must be refused"
    );
}

#[test]
fn l8_balanced_streams_are_accepted() {
    assert!(accepts(|b| {
        b.group_open();
        b.indent_open();
        b.text("a");
        b.softline();
        b.if_break(|b| b.char(b','), |_| {});
        b.indent_close();
        b.group_close();
        b.wrap(b'[', b']', |b| b.text("x"));
    }));
}
