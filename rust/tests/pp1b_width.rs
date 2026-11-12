//! sk-v27 LW3.PP-1b: display width.
//!
//! Width is display columns ([`unicode_width`], UAX #11: East Asian Wide and
//! Fullwidth = 2, combining and other zero-width marks = 0, emoji sequences
//! = 2), in the oracle's [W] `length s` and in the renderer alike. A
//! multi-line leaf has the width of its last line, and its newline is a hard
//! line at column 0 (it sets `HAS_HARD`).
//!
//! The falsifiers of the order's row:
//!
//! - CJK, emoji and combining-mark leaves: under the oracle no line overflows
//!   `w` past a breakable point (L7(i) in display columns), and the renderer
//!   agrees with the oracle. At entry the oracle measured `char`s (a CJK or
//!   emoji leaf overflowed) and the renderer measured bytes (it diverged).
//! - The oracle and the renderer agree on a Unicode generator at 10⁵ cases,
//!   multi-line leaves included.
//!
//! `pp1b_ascii_path.rs` reads the ASCII fast path against the general path.

mod oracle;

use oracle::lower::oracle_render;
use pprint::{Doc, FmtBuilder, PRINTER, Printer, render};
use unicode_width::UnicodeWidthStr;

const CASES: usize = 100_000;

fn cases() -> usize {
    std::env::var("PP_LAW_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CASES)
}

fn p(w: usize) -> Printer {
    Printer {
        max_width: w,
        ..PRINTER
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
}

// Words carry no space and no tab, and each starts with a base character, so
// no width rule joins two adjacent leaves (a leaf is measured on its own).
/// East Asian Wide and Fullwidth: 2 columns each, 3 bytes each, 1 `char`.
const CJK: &[&str] = &["漢", "日本", "中文字", "한국어", "Ｈｉ", "ｶﾅ", "語x"];
/// Emoji: presentation, modifier and ZWJ sequences, 2 columns each.
const EMOJI: &[&str] = &[
    "😀",
    "👍🏽",
    "👨\u{200D}👩\u{200D}👧",
    "❤\u{FE0F}",
    "a😀",
    "🚀🚀",
];
/// Combining and zero-width marks: 0 columns each, 2–3 bytes each.
const COMBINING: &[&str] = &[
    "e\u{301}",
    "a\u{300}\u{301}",
    "n\u{303}o",
    "\u{915}\u{94D}\u{937}",
    "x\u{200B}y",
    "o\u{308}\u{332}",
];
const ASCII: &[&str] = &["a", "bb", "ccc", "x;", "(y)"];
/// Multi-line leaves: the width of the last line, and `HAS_HARD`.
const MULTI: &[&str] = &["ab\ncd", "漢\nx", "p\n", "\nq", "e\u{301}\n😀\nzz", "a  \nb"];

#[derive(Debug, Clone)]
enum G {
    Text(&'static str),
    Quoted(&'static str),
    Softline,
    BreakLine,
    Hardline,
    CommaSep,
    FillSep,
    Group(Vec<G>),
    Indent(Vec<G>),
    Wrap(Vec<G>),
    IfBreak(Vec<G>, Vec<G>),
}

fn emit(b: &mut FmtBuilder<'static>, g: &G) {
    match g {
        G::Text(s) => b.text(s),
        G::Quoted(s) => b.quoted_text(s),
        G::Softline => b.softline(),
        G::BreakLine => b.break_line(),
        G::Hardline => b.hardline(),
        G::CommaSep => b.comma_sep(),
        G::FillSep => b.fill_sep(" "),
        G::Group(xs) => b.group(|b| xs.iter().for_each(|x| emit(b, x))),
        G::Indent(xs) => b.indent(|b| xs.iter().for_each(|x| emit(b, x))),
        G::Wrap(xs) => b.wrap(b'[', b']', |b| xs.iter().for_each(|x| emit(b, x))),
        G::IfBreak(t, e) => b.if_break(
            |b| t.iter().for_each(|x| emit(b, x)),
            |b| e.iter().for_each(|x| emit(b, x)),
        ),
    }
}

fn build(gs: &[G]) -> Doc<'static> {
    let mut b = FmtBuilder::new();
    gs.iter().for_each(|g| emit(&mut b, g));
    b.finish().expect("closure-built programs are balanced")
}

fn gen_seq(r: &mut Rng, words: &[&'static str], hard: bool, depth: u32) -> Vec<G> {
    (0..1 + r.below(4))
        .map(|_| gen_one(r, words, hard, depth))
        .collect()
}

fn gen_one(r: &mut Rng, words: &[&'static str], hard: bool, depth: u32) -> G {
    let k = r.below(if depth == 0 { 10 } else { 14 });
    let d = depth.saturating_sub(1);
    match k {
        0..=3 => G::Text(r.pick(words)),
        4 => G::Quoted(r.pick(words)),
        5 | 6 => G::Softline,
        7 => G::CommaSep,
        8 => G::FillSep,
        9 => {
            if hard {
                G::Hardline
            } else {
                G::BreakLine
            }
        }
        10 | 11 => G::Group(gen_seq(r, words, hard, d)),
        12 => {
            if r.chance(1, 2) {
                G::Indent(gen_seq(r, words, hard, d))
            } else {
                G::Wrap(gen_seq(r, words, hard, d))
            }
        }
        _ => G::IfBreak(gen_seq(r, words, hard, d), gen_seq(r, words, hard, d)),
    }
}

/// A document; `wrap_root` puts it in one outer group, so that every line op
/// sits in a group (what L7(i) reads), else half the documents read pprint's
/// flat depth-0 convention too.
fn gen_doc(r: &mut Rng, words: &[&'static str], hard: bool, wrap_root: bool) -> Vec<G> {
    let depth = 1 + r.below(3) as u32;
    let body = gen_seq(r, words, hard, depth);
    if wrap_root || r.chance(1, 2) {
        vec![G::Group(body)]
    } else {
        body
    }
}

/// L7(i) in display columns: a line wider than `w` holds no space after its
/// indentation (words carry none, so such a space is a flat line op whose
/// group [W] §4 `fits` admitted only if the line fit).
fn no_overflow_past_a_break(out: &str, w: usize, who: &str, gs: &[G]) {
    for ln in out.split('\n') {
        if ln.width() > w {
            assert!(
                !ln.trim_start().contains(' '),
                "{who} w={w}: line {ln:?} is {} columns and holds a flat break\nprogram: {gs:?}",
                ln.width()
            );
        }
    }
}

/// The class falsifier: `CASES` cells of programs over `words`.
fn class(seed: u64, words: &[&'static str], name: &str) {
    let mut r = Rng(seed);
    let mut cells = 0;
    while cells < cases() {
        let gs = gen_doc(&mut r, words, false, true);
        let doc = build(&gs);
        for _ in 0..4 {
            let w = r.below(41) as usize;
            let oracle = oracle_render(doc.ops(), p(w)).expect("lowerable");
            no_overflow_past_a_break(&oracle, w, &format!("{name}: the oracle"), &gs);
            let ours = render(&doc, p(w));
            assert_eq!(
                ours, oracle,
                "{name}: the renderer diverges from the oracle at w={w}\nprogram: {gs:?}"
            );
            no_overflow_past_a_break(&ours, w, &format!("{name}: the renderer"), &gs);
            cells += 1;
        }
    }
    eprintln!("{name}: {cells} cells, 0 divergences");
}

#[test]
fn cjk_leaves_do_not_overflow() {
    class(0x1b_0001, CJK, "CJK");
}

#[test]
fn emoji_leaves_do_not_overflow() {
    class(0x1b_0002, EMOJI, "emoji");
}

#[test]
fn combining_mark_leaves_do_not_overflow() {
    class(0x1b_0003, COMBINING, "combining");
}

/// The oracle and the renderer agree on a Unicode generator: every class,
/// ASCII and multi-line leaves, hard lines included, 10⁵ cells.
#[test]
fn unicode_oracle_and_renderer_agree() {
    let all: Vec<&'static str> = [CJK, EMOJI, COMBINING, ASCII, MULTI].concat();
    let mut r = Rng(0x1b_0004);
    let mut cells = 0;
    while cells < cases() {
        let hard = r.chance(1, 3);
        let gs = gen_doc(&mut r, &all, hard, false);
        let doc = build(&gs);
        for _ in 0..4 {
            let w = r.below(41) as usize;
            let oracle = oracle_render(doc.ops(), p(w)).expect("lowerable");
            let ours = render(&doc, p(w));
            assert_eq!(
                ours, oracle,
                "Unicode: the renderer diverges from the oracle at w={w}\nprogram: {gs:?}"
            );
            cells += 1;
        }
    }
    eprintln!("Unicode: {cells} cells, 0 divergences");
}

/// Pinned by hand and by the oracle: a multi-line leaf has the width of its
/// last line and sets `HAS_HARD`; its newline is at column 0 (no indent) and
/// trailing spaces before it are removed, as before every newline.
#[test]
fn multiline_leaf_last_line_width_and_has_hard() {
    let check = |gs: &[G], w: usize, expected: &str| {
        let doc = build(gs);
        let oracle = oracle_render(doc.ops(), p(w)).expect("lowerable");
        assert_eq!(oracle, expected, "the oracle, w={w}, {gs:?}");
        assert_eq!(render(&doc, p(w)), expected, "the renderer, w={w}, {gs:?}");
    };
    // HAS_HARD: the group breaks at any width.
    let g = [G::Group(vec![G::Text("ab\ncd"), G::Softline, G::Text("e")])];
    check(&g, 80, "ab\ncd\ne");
    // The column after the leaf is its last line's width (1, not 4).
    let g = [
        G::Text("xy\nz"),
        G::Group(vec![G::Text("aa"), G::Softline, G::Text("bb")]),
    ];
    check(&g, 6, "xy\nzaa bb");
    check(&g, 5, "xy\nzaa\nbb");
    // A group's rest stops at the leaf's first newline (L3′).
    let g = [
        G::Group(vec![G::Text("aa"), G::Softline, G::Text("b")]),
        G::Text("cc\nddd"),
    ];
    check(&g, 6, "aa bcc\nddd");
    check(&g, 5, "aa\nbcc\nddd");
    // No indentation after a leaf's own newline; trailing spaces removed.
    let g = [G::Indent(vec![G::Hardline, G::Text("p  \nq")])];
    check(&g, 80, "\n    p\nq");
    // Display width: 漢 is 2 columns, so the second line is 2 + 1 + 2.
    let g = [G::Group(vec![
        G::Text("x\n漢"),
        G::Group(vec![G::Softline, G::Text("漢")]),
    ])];
    check(&g, 5, "x\n漢 漢");
    check(&g, 4, "x\n漢\n漢");
}
