//! sk-v27 LW3.PP-2 (U4b trivia) and ruling S8-B (`FillSep`): the falsifier.
//!
//! - L9, trivia: a `LineSuffix` is flushed before the next structural
//!   newline; a `LineSuffixBoundary` forces that flush through a hard line
//!   when a suffix is pending; at most one blank line in a row.
//! - L2 for `BreakParent`: it forces every enclosing group to break.
//! - S8-B: `fill_sep` is one `FillSep` op, defined as Leijen's composition
//!   (`softline = group line`, generalized to k spaces) and equal to it.
//!
//! Each property reads at least 10⁵ cells (one cell = one document at one
//! width), seeds fixed. The reference is the oracle (`tests/oracle/`),
//! built here directly from a test-side document tree with the oracle's own
//! combinators (Wadler, Leijen, [P], and the L9 extension of `be`), never
//! through `render.rs` or the op stream. The builder side goes through
//! [`bind`], the one place the trivia constructs meet the public API.

mod oracle;

use oracle::*;
use pprint::{FmtBuilder, PRINTER, Printer, render};

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

/// The trivia constructs on the builder (LW3.PP-2's public API).
mod bind {
    use pprint::FmtBuilder;

    pub fn suffix(b: &mut FmtBuilder<'static>, s: &'static str) {
        b.line_suffix(|b| b.text(s));
    }

    pub fn boundary(b: &mut FmtBuilder<'static>) {
        b.line_suffix_boundary();
    }

    pub fn break_parent(b: &mut FmtBuilder<'static>) {
        b.break_parent();
    }
}

// ── A deterministic generator (SplitMix64) ──────────────────────────────────

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
}

const WORDS: [&str; 6] = ["a", "bb", "ccc", "dddd", "x;", "(y)"];
/// Suffix contents: each a comment, distinct, starting with a space.
const SUFFIXES: [&str; 6] = [" //1", " //22", " /*3*/", " #4", " //55555", " ;6"];
const SPACES: [&str; 3] = ["", " ", "  "];

/// A document tree, read by both sides.
#[derive(Debug, Clone)]
enum T {
    Text(&'static str),
    /// A soft line; only generated inside a group (pprint flattens line ops
    /// outside every group, a convention the oracle lowering mirrors).
    Softline,
    Hardline,
    Group(Vec<T>),
    Indent(Vec<T>),
    IfBreak(Vec<T>, Vec<T>),
    FillSep(usize),
    Suffix(usize),
    Boundary,
    BreakParent,
}

fn emit(b: &mut FmtBuilder<'static>, t: &T) {
    match t {
        T::Text(s) => b.text(s),
        T::Softline => b.softline(),
        T::Hardline => b.hardline(),
        T::Group(xs) => b.group(|b| xs.iter().for_each(|x| emit(b, x))),
        T::Indent(xs) => b.indent(|b| xs.iter().for_each(|x| emit(b, x))),
        T::IfBreak(t, e) => b.if_break(
            |b| t.iter().for_each(|x| emit(b, x)),
            |b| e.iter().for_each(|x| emit(b, x)),
        ),
        T::FillSep(k) => b.fill_sep(SPACES[*k]),
        T::Suffix(i) => bind::suffix(b, SUFFIXES[*i]),
        T::Boundary => bind::boundary(b),
        T::BreakParent => bind::break_parent(b),
    }
}

fn seq(parts: Vec<D>) -> D {
    parts
        .into_iter()
        .rev()
        .fold(nil(), |acc, d| match &*acc {
            Doc::Nil => d,
            _ => cat(d, acc),
        })
}

/// The oracle's document for `ts`, from the oracle's combinators.
fn to_oracle(ts: &[T]) -> D {
    seq(ts.iter().map(one).collect())
}

fn one(t: &T) -> D {
    match t {
        T::Text(s) => text(s),
        T::Softline => line(),
        T::Hardline => hardline(),
        T::Group(xs) => group(to_oracle(xs)),
        T::Indent(xs) => nest(PRINTER.indent as isize, to_oracle(xs)),
        T::IfBreak(t, e) => flat_alt(to_oracle(t), to_oracle(e)),
        // S8-B: [L] `softline = group line`, generalized to k spaces.
        T::FillSep(k) => {
            let flat = if *k == 0 {
                nil()
            } else {
                text(SPACES[*k])
            };
            group(flat_alt(linebreak(), flat))
        }
        T::Suffix(i) => line_suffix(text(SUFFIXES[*i])),
        T::Boundary => boundary(),
        T::BreakParent => break_parent(),
    }
}

/// Trivia weights: a document with suffixes, boundaries and break-parents
/// in trailing regions of groups, across hard lines and fill separators.
fn gen_seq(r: &mut Rng, depth: u32, in_group: bool, len: u64) -> Vec<T> {
    (0..1 + r.below(len))
        .map(|_| gen_one(r, depth, in_group))
        .collect()
}

fn gen_one(r: &mut Rng, depth: u32, in_group: bool) -> T {
    let d = depth.saturating_sub(1);
    let k = r.below(if depth == 0 { 9 } else { 13 });
    match k {
        0 | 1 => T::Text(WORDS[r.below(WORDS.len() as u64) as usize]),
        2 if in_group => T::Softline,
        2 => T::Text("w"),
        3 => T::Suffix(r.below(SUFFIXES.len() as u64) as usize),
        4 => T::Boundary,
        5 => {
            if r.chance(1, 3) {
                T::Hardline
            } else {
                T::BreakParent
            }
        }
        6 => T::FillSep(r.below(3) as usize),
        7 => T::Suffix(r.below(SUFFIXES.len() as u64) as usize),
        8 => T::Text(WORDS[r.below(WORDS.len() as u64) as usize]),
        9 | 10 => T::Group(gen_seq(r, d, true, 4)),
        11 => T::Indent(gen_seq(r, d, in_group, 3)),
        _ if in_group => T::IfBreak(gen_seq(r, d, true, 2), gen_seq(r, d, true, 2)),
        _ => T::Group(gen_seq(r, d, true, 3)),
    }
}

fn gen_doc(r: &mut Rng) -> Vec<T> {
    let depth = 1 + r.below(4) as u32;
    let body = gen_seq(r, depth, false, 6);
    if r.chance(1, 2) {
        vec![T::Group(body)]
    } else {
        body
    }
}

fn build(ts: &[T]) -> pprint::Doc<'static> {
    let mut b = FmtBuilder::new();
    ts.iter().for_each(|t| emit(&mut b, t));
    b.finish().expect("closure-built trees are balanced")
}

fn oracle_out(ts: &[T], w: usize) -> String {
    pretty_pprint(w as isize, &to_oracle(ts)).expect("no SFail on built trees")
}

// ── The pinned case ────────────────────────────────────────────────────────

/// A suffix pending across a hard line. The group `a <suffix> line b` is flat
/// at w = 80 (a suffix is zero width), so its line is a space and the
/// suffix waits for the next newline, the hard line after the group:
/// `a b //1` then `z`. Emitting the comment in place gives `a //1 b`.
#[test]
fn a_pending_suffix_across_a_hardline_matches_the_oracle() {
    let ts = vec![
        T::Group(vec![T::Text("a"), T::Suffix(0), T::Softline, T::Text("b")]),
        T::Hardline,
        T::Text("z"),
    ];
    assert_eq!(oracle_out(&ts, 80), "a b //1\nz");
    assert_eq!(render(&build(&ts), p(80)), "a b //1\nz");
}

/// A boundary with a suffix pending is a hard line that flushes it: inside a
/// flat group, `a <suffix> <boundary> b` is `a //1` then `b`.
#[test]
fn a_boundary_flushes_a_pending_suffix_through_a_hardline() {
    let ts = vec![T::Group(vec![
        T::Text("a"),
        T::Suffix(0),
        T::Boundary,
        T::Text("b"),
    ])];
    assert_eq!(oracle_out(&ts, 80), "a //1\nb");
    assert_eq!(render(&build(&ts), p(80)), "a //1\nb");
    // No suffix pending: the boundary is nothing.
    let ts = vec![T::Group(vec![T::Text("a"), T::Boundary, T::Text("b")])];
    assert_eq!(render(&build(&ts), p(80)), "ab");
}

/// `BreakParent` breaks its group and every enclosing one at any width.
#[test]
fn break_parent_breaks_every_enclosing_group() {
    let ts = vec![T::Group(vec![
        T::Text("a"),
        T::Softline,
        T::Group(vec![T::Text("b"), T::Softline, T::Text("c"), T::BreakParent]),
    ])];
    assert_eq!(oracle_out(&ts, 1000), "a\nb\nc");
    assert_eq!(render(&build(&ts), p(1000)), "a\nb\nc");
}

// ── L9 against the oracle, ≥ 10⁵ cells ─────────────────────────────────────

/// G2 with trivia generators: the renderer equals the oracle on documents
/// that place suffixes, boundaries, break-parents and fill separators in
/// the trailing regions of groups, across hard lines, at widths 0–40.
#[test]
fn l9_trivia_documents_agree_with_the_oracle() {
    let mut r = Rng(0x19_0009);
    let mut cells = 0;
    while cells < cases() {
        let ts = gen_doc(&mut r);
        let doc = build(&ts);
        for _ in 0..4 {
            let w = r.below(41) as usize;
            let ours = render(&doc, p(w));
            let theirs = oracle_out(&ts, w);
            assert_eq!(ours, theirs, "L9: diverges from the oracle at w={w}\ntree: {ts:?}");
            cells += 1;
        }
    }
    eprintln!("L9 (oracle): {cells} cells, 0 divergences");
}

/// L9(i): a suffix is flushed before the next newline. Every suffix in the
/// output ends its line or is followed only by other suffixes up to the
/// newline (suffixes are the only text starting with ` //`, ` /*`, ` #` or
/// ` ;`), and every suffix the document holds appears exactly once.
#[test]
fn l9_i_a_suffix_ends_its_line() {
    let mut r = Rng(0x19_0091);
    let mut cells = 0;
    while cells < cases() {
        let ts = gen_doc(&mut r);
        if suffix_in_if_break(&ts, false) {
            continue;
        }
        let doc = build(&ts);
        let mut want = 0usize;
        count_suffixes(&ts, &mut want);
        for _ in 0..4 {
            let w = r.below(41) as usize;
            let out = render(&doc, p(w));
            let mut seen = 0usize;
            for ln in out.split('\n') {
                let mut rest = ln;
                // Strip trailing suffixes; what is left may hold none.
                while let Some(s) = SUFFIXES.iter().find(|s| rest.ends_with(*s)) {
                    rest = &rest[..rest.len() - s.len()];
                    seen += 1;
                }
                for s in SUFFIXES {
                    assert!(
                        !rest.contains(s),
                        "L9(i) w={w}: a suffix is not at the end of its line {ln:?}\ntree: {ts:?}"
                    );
                }
            }
            assert_eq!(seen, want, "L9(i) w={w}: a suffix was lost or doubled\n{out:?}");
            cells += 1;
        }
    }
    eprintln!("L9(i): {cells} cells");
}

fn suffix_in_if_break(ts: &[T], inside: bool) -> bool {
    ts.iter().any(|t| match t {
        T::Suffix(_) => inside,
        T::Group(xs) | T::Indent(xs) => suffix_in_if_break(xs, inside),
        T::IfBreak(a, b) => suffix_in_if_break(a, true) || suffix_in_if_break(b, true),
        _ => false,
    })
}

fn count_suffixes(ts: &[T], n: &mut usize) {
    for t in ts {
        match t {
            T::Suffix(_) => *n += 1,
            T::Group(xs) | T::Indent(xs) => count_suffixes(xs, n),
            // The two branches of an `IfBreak` are alternatives; the law
            // skips trees with a suffix inside one.
            _ => {}
        }
    }
}

/// L9(iii): at most one blank line in a row, however many hard lines the
/// document puts together.
#[test]
fn l9_iii_at_most_one_blank_line() {
    let mut r = Rng(0x19_0093);
    let mut cells = 0;
    while cells < cases() {
        let mut ts = gen_doc(&mut r);
        // A run of 2–5 hard lines, somewhere.
        let at = r.below(ts.len() as u64 + 1) as usize;
        let run = 2 + r.below(4) as usize;
        for _ in 0..run {
            ts.insert(at, T::Hardline);
        }
        let doc = build(&ts);
        for _ in 0..4 {
            let w = r.below(41) as usize;
            let out = render(&doc, p(w));
            assert!(
                !out.contains("\n\n\n") && !out.starts_with("\n\n"),
                "L9(iii) w={w}: two blank lines in a row\n{out:?}\ntree: {ts:?}"
            );
            assert_eq!(out, oracle_out(&ts, w), "L9(iii) w={w}: oracle\ntree: {ts:?}");
            cells += 1;
        }
    }
    eprintln!("L9(iii): {cells} cells");
}

// ── S8-B: `FillSep` is Leijen's composition ────────────────────────────────

/// `fill_sep` builds one `FillSep` op (the op set carries it, S8-B), and a
/// `linear_join` renders exactly as [L] `fillSep = fold (</>)` with `x </> y
/// = x <> softline <> y` and `softline = group line` over the same items,
/// words or groups, at every width tested.
#[test]
fn s8b_fill_sep_is_one_op_equal_to_leijen_fill_sep() {
    let mut b = FmtBuilder::new();
    b.text("a");
    b.fill_sep(" ");
    b.text("b");
    let doc = b.finish().unwrap();
    assert!(
        doc.ops().iter().any(|op| format!("{op:?}").starts_with("FillSep")),
        "S8-B: fill_sep builds no FillSep op: {:?}",
        doc.ops()
    );

    let mut r = Rng(0x15_8b01);
    let mut cells = 0;
    while cells < cases() {
        let n = 1 + r.below(7) as usize;
        let items: Vec<T> = (0..n)
            .map(|_| {
                if r.chance(2, 3) {
                    T::Text(WORDS[r.below(WORDS.len() as u64) as usize])
                } else {
                    T::Group(vec![
                        T::Text(WORDS[r.below(4) as usize]),
                        T::Softline,
                        T::Text(WORDS[r.below(4) as usize]),
                    ])
                }
            })
            .collect();
        let mut b = FmtBuilder::new();
        b.linear_join(items.clone(), " ", |b, t| emit(b, &t));
        let doc = b.finish().unwrap();
        let leijen = fill_sep(&items.iter().map(one).collect::<Vec<_>>());
        for _ in 0..4 {
            let w = r.below(41) as usize;
            assert_eq!(
                render(&doc, p(w)),
                pretty_pprint(w as isize, &leijen).unwrap(),
                "S8-B w={w}: items {items:?}"
            );
            cells += 1;
        }
    }
    eprintln!("S8-B (Leijen fillSep): {cells} cells, 0 divergences");
}

/// The `FillSep` op renders byte-equal to the composition it fuses,
/// `group (if_break (break_line) (text ws))`, on documents that put fill
/// separators after groups, inside indents and next to trivia.
#[test]
fn s8b_fill_sep_equals_its_composition() {
    let mut r = Rng(0x15_8b02);
    let mut cells = 0;
    while cells < cases() {
        let ts = gen_doc(&mut r);
        let doc = build(&ts);
        let mut b = FmtBuilder::new();
        ts.iter().for_each(|t| emit_composed(&mut b, t));
        let composed = b.finish().unwrap();
        for _ in 0..4 {
            let w = r.below(41) as usize;
            assert_eq!(
                render(&doc, p(w)),
                render(&composed, p(w)),
                "S8-B w={w}: the op and its composition differ\ntree: {ts:?}"
            );
            cells += 1;
        }
    }
    eprintln!("S8-B (composition): {cells} cells, 0 divergences");
}

fn emit_composed(b: &mut FmtBuilder<'static>, t: &T) {
    match t {
        T::FillSep(k) => {
            let ws = SPACES[*k];
            b.group(|b| b.if_break(|b| b.break_line(), |b| b.str(ws)));
        }
        T::Group(xs) => b.group(|b| xs.iter().for_each(|x| emit_composed(b, x))),
        T::Indent(xs) => b.indent(|b| xs.iter().for_each(|x| emit_composed(b, x))),
        T::IfBreak(t, e) => b.if_break(
            |b| t.iter().for_each(|x| emit_composed(b, x)),
            |b| e.iter().for_each(|x| emit_composed(b, x)),
        ),
        other => emit(b, other),
    }
}
