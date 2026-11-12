//! sk-v27 LW3.PP-1a: the one-loop renderer.
//!
//! The falsifiers of the order's row, as tests:
//!
//! - `char(0xFF)` is refused at build (`finish()` is `Err`); it never renders.
//!   At entry it rendered the byte into a `String` through
//!   `from_utf8_unchecked`, i.e. invalid UTF-8.
//! - The other ASCII-only byte fields (`wrap`'s delimiters, `sep`'s bytes) and
//!   a tab in any leaf are refused the same way (ABI-v2.md, LW3 addendum).
//! - L1 holds at any nesting depth: a flat group 65 deep stays flat (at entry
//!   the u64 mode stack read every group past depth 64 as broken), and L1
//!   holds at depth 10⁴, checked against the closed form of [W] §4 `fits`
//!   (and against the oracle itself at depths it can reach).
//!
//! The 10⁶-deep `IfBreak` nest is in `pp1a_deep_if_break.rs`: at entry it
//! overflows the stack and aborts its whole test binary.

mod oracle;

use oracle::lower::oracle_render;
use pprint::{Doc, FmtBuilder, PRINTER, Printer, render};

fn p(w: usize) -> Printer {
    Printer {
        max_width: w,
        ..PRINTER
    }
}

/// `finish()` must refuse; if it does not, report what the accepted stream
/// renders to (at entry: bytes that are not UTF-8).
fn assert_refused(b: FmtBuilder<'_>, what: &str) {
    match b.finish() {
        Err(_) => {}
        Ok(doc) => {
            let out = render(&doc, PRINTER);
            let utf8 = std::str::from_utf8(out.as_bytes()).is_ok();
            panic!(
                "{what}: accepted at build; renders {:?} (valid UTF-8: {utf8})",
                out.as_bytes()
            );
        }
    }
}

#[test]
fn char_0xff_is_refused_at_build() {
    let mut b = FmtBuilder::new();
    b.text("a");
    b.char(0xFF);
    match b.finish() {
        Err(e) => assert!(
            e.to_string().contains("ASCII"),
            "char(0xFF): refused for the wrong reason: {e}"
        ),
        Ok(doc) => {
            let out = render(&doc, PRINTER);
            assert!(
                std::str::from_utf8(out.as_bytes()).is_ok(),
                "char(0xFF) renders invalid UTF-8: {:?}",
                out.as_bytes()
            );
            panic!("char(0xFF) accepted at build");
        }
    }
}

#[test]
fn every_non_ascii_char_byte_is_refused() {
    for c in 0x80u8..=0xFF {
        let mut b = FmtBuilder::new();
        b.char(c);
        assert_refused(b, &format!("char({c:#04x})"));
    }
}

#[test]
fn non_ascii_wrap_and_sep_bytes_are_refused() {
    let mut b = FmtBuilder::new();
    b.wrap(0xC3, b']', |b| b.text("x"));
    assert_refused(b, "wrap open 0xC3");

    let mut b = FmtBuilder::new();
    b.wrap(b'[', 0xA9, |b| b.text("x"));
    assert_refused(b, "wrap close 0xA9");

    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("x");
        b.sep("\u{e9}", ",");
        b.text("y");
    });
    assert_refused(b, "sep flat é");

    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("x");
        b.sep(" | ", "\u{a7}");
        b.text("y");
    });
    assert_refused(b, "sep brk §");
}

#[test]
fn a_tab_in_a_leaf_is_refused() {
    let mut b = FmtBuilder::new();
    b.char(b'\t');
    assert_refused(b, "char tab");

    let mut b = FmtBuilder::new();
    b.text("a\tb");
    assert_refused(b, "text with a tab");

    let mut b = FmtBuilder::new();
    b.text_owned("a\tb");
    assert_refused(b, "owned text with a tab");

    let mut b = FmtBuilder::new();
    b.quoted_text("a\tb");
    assert_refused(b, "quoted text with a tab");

    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("x");
        b.sep("\t", ",");
    });
    assert_refused(b, "sep with a tab");
}

#[test]
fn ascii_bytes_still_render() {
    let mut b = FmtBuilder::new();
    b.wrap(b'[', b']', |b| {
        b.char(b'a');
        b.sep(" | ", "|");
        b.char(b'~');
    });
    let doc = b.finish().unwrap();
    assert_eq!(render(&doc, PRINTER), "[a | ~]");
    assert_eq!(render(&doc, p(3)), "[\n    a|\n    ~\n]");
}

/// `n` nested groups, each `( softline <inner> softline )`, innermost `a`.
fn nest_parens(n: usize) -> Doc<'static> {
    let mut b = FmtBuilder::new();
    for _ in 0..n {
        b.group_open();
        b.char(b'(');
        b.softline();
    }
    b.char(b'a');
    for _ in 0..n {
        b.softline();
        b.char(b')');
        b.group_close();
    }
    b.finish().unwrap()
}

/// L1 in closed form for `nest_parens(n)` at width `w`. Group `G_i` (depth
/// i, 0-based) has flat width `4(n − i) + 1`. In a broken parent each softline
/// is a newline at indent 0, so `G_i` starts at column 0 and its rest up to
/// the next newline is empty: by [W] §4 `fits` it is flat iff
/// `4(n − i) + 1 <= w`, and then everything inside it is flat ([W] §3
/// `flatten (x :<|> y) = flatten x`).
fn nest_parens_expected(n: usize, w: usize) -> String {
    let flat = |i: usize| 4 * (n - i) + 1;
    let k = (0..=n).find(|&i| i == n || flat(i) <= w).unwrap();
    let mut s = String::new();
    for _ in 0..k {
        s.push_str("(\n");
    }
    // Levels k..n flat.
    for _ in k..n {
        s.push_str("( ");
    }
    s.push('a');
    for _ in k..n {
        s.push_str(" )");
    }
    for _ in 0..k {
        s.push_str("\n)");
    }
    s
}

#[test]
fn a_flat_group_65_deep_stays_flat() {
    let n = 65;
    let doc = nest_parens(n);
    let w = 4 * n + 1;
    let got = render(&doc, p(w));
    assert!(
        !got.contains('\n'),
        "L1: a 65-deep group that fits ({w} columns) breaks: {got:?}"
    );
    assert_eq!(got, nest_parens_expected(n, w));
}

/// The closed form agrees with the oracle where the oracle can go.
#[test]
fn closed_form_matches_the_oracle() {
    for n in [1, 2, 7, 33, 64, 65, 70] {
        let doc = nest_parens(n);
        for w in [0, 1, 4, 5, 9, 40, 4 * n, 4 * n + 1, 4 * n + 5] {
            let expected = nest_parens_expected(n, w);
            let oracle = oracle_render(doc.ops(), p(w)).unwrap();
            assert_eq!(oracle, expected, "oracle vs closed form, n={n} w={w}");
            assert_eq!(render(&doc, p(w)), expected, "renderer, n={n} w={w}");
        }
    }
}

#[test]
fn l1_holds_at_depth_10_000() {
    let n = 10_000;
    let doc = nest_parens(n);
    for w in [usize::MAX, 4 * n + 1, 4 * n, 4 * 5_000 + 1, 4 * 64 + 1, 80, 5, 0] {
        let got = render(&doc, p(w));
        let expected = nest_parens_expected(n, w.min(u32::MAX as usize / 2));
        assert!(got == expected, "L1 at depth 10^4 diverges at w={w}");
    }
}
