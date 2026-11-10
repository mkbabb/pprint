//! sk-v27 LW2.PP-3: laws L1–L8 (design `pprint.md` §B.2) as property tests
//! of the renderer against the Wadler/Leijen oracle (`tests/oracle/`).
//!
//! Each law has its own generator, weighted toward the constructs the law is
//! about, and each test names the equations it checks. A *case* is one
//! (document, width) cell; every law reads at least `CASES` = 10⁵ of them
//! (override with `PP_LAW_CASES`, never below the floor in CI). Seeds are
//! fixed, so a reading is reproducible byte for byte.
//!
//! The renderer and the oracle share nothing but the public `FmtOp` enum:
//! the oracle lowers the op stream to a tree (`oracle::lower`) and decides
//! every group itself with [W] §4 `be`/`better`/`fits`. Group headers are
//! not read by the oracle.

mod oracle;

use oracle::lower::{lower, oracle_render};
use oracle::*;
use pprint::{Doc, FmtBuilder, PRINTER, Printer, render};

/// The floor of cases per law (the order's "≥ 10⁵ cases per law").
const CASES: usize = 100_000;

fn cases() -> usize {
    std::env::var("PP_LAW_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CASES)
}

/// The width the renderer treats as infinite (ABI v2 clamps `max_width` to
/// `u32::MAX / 2`).
const W_INF: usize = (u32::MAX / 2) as usize;

fn p(w: usize) -> Printer {
    Printer {
        max_width: w,
        ..PRINTER
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

/// Words carry no space, so a space in the output can only come from a line
/// op rendered flat (what L7(i) reads).
const WORDS: [&str; 8] = ["a", "bb", "ccc", "dddd", "eeeee", "ffffff", "x;", "(y)"];

/// A builder program: a tree of `FmtBuilder` calls.
#[derive(Debug, Clone)]
enum G {
    Text(&'static str),
    Char(u8),
    Int(i64),
    Softline,
    BreakLine,
    Hardline,
    CommaSep,
    /// `sep(" | ", "|")`: the break form carries no space.
    Sep,
    FillSep(&'static str),
    Group(Vec<G>),
    Indent(Vec<G>),
    Wrap(u8, u8, Vec<G>),
    IfBreak(Vec<G>, Vec<G>),
}

fn emit(b: &mut FmtBuilder<'static>, g: &G) {
    match g {
        G::Text(s) => b.text(s),
        G::Char(c) => b.char(*c),
        G::Int(v) => b.i64(*v),
        G::Softline => b.softline(),
        G::BreakLine => b.break_line(),
        G::Hardline => b.hardline(),
        G::CommaSep => b.comma_sep(),
        G::Sep => b.sep(" | ", "|"),
        G::FillSep(s) => b.fill_sep(s),
        G::Group(xs) => b.group(|b| xs.iter().for_each(|x| emit(b, x))),
        G::Indent(xs) => b.indent(|b| xs.iter().for_each(|x| emit(b, x))),
        G::Wrap(o, c, xs) => b.wrap(*o, *c, |b| xs.iter().for_each(|x| emit(b, x))),
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

/// Per-law weights over the constructors. Leaves and lines are always
/// available; the structural weights are what each law turns up.
#[derive(Clone, Copy)]
struct Mix {
    hard: u64,
    group: u64,
    indent: u64,
    wrap: u64,
    if_break: u64,
    fill: u64,
    sep: u64,
}

impl Mix {
    const BASE: Mix = Mix {
        hard: 0,
        group: 4,
        indent: 1,
        wrap: 1,
        if_break: 1,
        fill: 1,
        sep: 1,
    };
}

fn gen_seq(r: &mut Rng, m: Mix, depth: u32, len: u64) -> Vec<G> {
    (0..1 + r.below(len))
        .map(|_| gen_one(r, m, depth))
        .collect()
}

fn gen_one(r: &mut Rng, m: Mix, depth: u32) -> G {
    // Structural constructors only below the depth bound.
    let s = |w: u64| if depth == 0 { 0 } else { w };
    let weights = [
        6, // leaf
        4, // line
        m.hard,
        m.fill,
        m.sep,
        s(m.group),
        s(m.indent),
        s(m.wrap),
        s(m.if_break),
    ];
    let mut k = r.below(weights.iter().sum());
    let pick = weights
        .iter()
        .position(|&w| {
            let hit = k < w;
            k = k.saturating_sub(w);
            hit
        })
        .expect("k < the sum of the weights");
    let d = depth.saturating_sub(1);
    match pick {
        0 => match r.below(6) {
            0 => G::Char(b",;:"[r.below(3) as usize]),
            1 => G::Int(r.below(2_000) as i64 - 1_000),
            _ => G::Text(WORDS[r.below(WORDS.len() as u64) as usize]),
        },
        1 => match r.below(4) {
            0 | 1 => G::Softline,
            2 => G::BreakLine,
            _ => G::CommaSep,
        },
        2 => G::Hardline,
        3 => G::FillSep([" ", ", ", ""][r.below(3) as usize]),
        4 => G::Sep,
        5 => G::Group(gen_seq(r, m, d, 4)),
        6 => G::Indent(gen_seq(r, m, d, 3)),
        7 => {
            let (o, c) = [(b'[', b']'), (b'{', b'}'), (b'(', b')')][r.below(3) as usize];
            G::Wrap(o, c, gen_seq(r, m, d, 4))
        }
        _ => {
            let t = if r.chance(1, 5) {
                vec![]
            } else {
                gen_seq(r, m, d, 2)
            };
            let e = if r.chance(1, 5) {
                vec![]
            } else {
                gen_seq(r, m, d, 2)
            };
            G::IfBreak(t, e)
        }
    }
}

/// A document: a sequence at depth ≤ 4, wrapped in one outer group unless
/// the law reads pprint's flat depth-0 convention too.
fn gen_doc(r: &mut Rng, m: Mix, wrap_root: bool) -> Vec<G> {
    let depth = 1 + r.below(4) as u32;
    let body = gen_seq(r, m, depth, 5);
    if wrap_root {
        vec![G::Group(body)]
    } else {
        body
    }
}

fn widths(r: &mut Rng, n: usize) -> Vec<usize> {
    (0..n).map(|_| r.below(41) as usize).collect()
}

/// Render through both sides and compare; returns the number of cells.
fn agree(gs: &[G], ws: &[usize], law: &str) -> usize {
    let doc = build(gs);
    for &w in ws {
        let ours = render(&doc, p(w));
        let oracle = oracle_render(doc.ops(), p(w)).expect("lowerable");
        assert_eq!(
            ours, oracle,
            "{law}: renderer diverges from the oracle at w={w}\nprogram: {gs:?}"
        );
    }
    ws.len()
}

/// The common driver: `CASES` cells, 4 widths per document.
fn run_law(seed: u64, m: Mix, wrap_root: bool, law: &str) {
    let mut r = Rng(seed);
    let mut cells = 0;
    while cells < cases() {
        let wrap = wrap_root || r.chance(1, 2);
        let gs = gen_doc(&mut r, m, wrap);
        let ws = widths(&mut r, 4);
        cells += agree(&gs, &ws, law);
    }
    eprintln!("{law}: {cells} cells, 0 divergences");
}

// ── L1 ──────────────────────────────────────────────────────────────────────

/// L1, group. [W] §3 `group x = flatten x :<|> x`; [W] §4 `be w k ((i,x :<|>
/// y):z) = better w k (be w k ((i,x):z)) (be w k ((i,y):z))` and `better w k
/// x y = if fits (w-k) x then x else y`: the flat side is chosen iff
/// `fits (w − col) (flatten d ++ rest)`.
#[test]
fn l1_group_flat_iff_fits() {
    let m = Mix {
        group: 6,
        indent: 0,
        wrap: 1,
        if_break: 0,
        fill: 0,
        sep: 0,
        hard: 0,
    };
    run_law(0x11_0001, m, false, "L1");
}

// ── L2 ──────────────────────────────────────────────────────────────────────

/// L2, hard. [P] `flatten hardline = Fail` and `fits _ SFail = False`, with
/// [P] `flatten (FlatAlt _ y) = flatten y`: a hard line forces its group and
/// every enclosing group to break, except across an `IfBreak` break branch.
/// Widths include 20,000 and ∞ (the v1 `+10000` sentinel's failure).
#[test]
fn l2_hard_forces_break() {
    let m = Mix {
        hard: 3,
        if_break: 2,
        ..Mix::BASE
    };
    let mut r = Rng(0x12_0002);
    let mut cells = 0;
    while cells < cases() {
        let wrap = r.chance(1, 2);
        let gs = gen_doc(&mut r, m, wrap);
        let mut ws = widths(&mut r, 2);
        ws.push(20_000);
        ws.push(W_INF);
        cells += agree(&gs, &ws, "L2");
    }
    eprintln!("L2: {cells} cells, 0 divergences");
}

// ── L3′ ─────────────────────────────────────────────────────────────────────

/// L3′, rest lookahead. [W] §4 `better` asks `fits` about `be w k ((i,x):z)`,
/// so the rest `z` counts up to its first `Line` (`fits w (i `Line` x) =
/// True`), with every later union resolved by its own `better` as `fits`
/// reads. Programs put text, later groups (with and without hard lines),
/// fill separators and `IfBreak`s after groups.
#[test]
fn l3_rest_lookahead() {
    let m = Mix {
        hard: 1,
        group: 6,
        indent: 1,
        wrap: 1,
        if_break: 2,
        fill: 2,
        sep: 1,
    };
    let mut r = Rng(0x13_0003);
    let mut cells = 0;
    while cells < cases() {
        // A group, then a trailing region built from the same generator.
        let mut gs = vec![G::Group(gen_seq(&mut r, m, 2, 4))];
        gs.extend(gen_seq(&mut r, m, 2, 4));
        let gs = if r.chance(1, 2) {
            vec![G::Group(gs)]
        } else {
            gs
        };
        let ws = widths(&mut r, 4);
        cells += agree(&gs, &ws, "L3'");
    }
    eprintln!("L3': {cells} cells, 0 divergences");
}

// ── L4 ──────────────────────────────────────────────────────────────────────

/// L4, nest. [W] §3 `nest i x = NEST i x`; [W] §4 `be w k ((i,NEST j x):z)
/// = be w k ((i+j,x):z)` and `be w k ((i,LINE):z) = i `Line` be w i z`:
/// nesting changes only the indentation after a newline, never a group's
/// flat width. `Indent` and `Wrap` lower to `nest`.
#[test]
fn l4_nest_changes_indentation_only() {
    let m = Mix {
        indent: 4,
        wrap: 4,
        group: 3,
        ..Mix::BASE
    };
    run_law(0x14_0004, m, false, "L4");
}

/// L4 for [L] `align` and `hang` on the oracle (the renderer has no align op
/// in ABI v2; `column`/`nesting` stay outside its instruction set): [L]
/// `align d = column (\k -> nesting (\i -> nest (k - i) d))` puts every line
/// of `d` at the column where `d` starts, and `hang i d = align (nest i d)`.
#[test]
fn l4_align_and_hang_on_the_oracle() {
    let mut r = Rng(0x14_0005);
    let mut cells = 0;
    while cells < cases() {
        let k = r.below(6) as usize;
        let i = r.below(4) as isize;
        let words: Vec<D> = (0..2 + r.below(4))
            .map(|_| text(WORDS[r.below(6) as usize]))
            .collect();
        let lead = "p".repeat(k);
        // vsep forces every line: each line of the aligned block starts at k.
        let d = cat(text(&lead), align(vsep(&words)));
        let out = pretty(80, &d).unwrap();
        for (n, ln) in out.split('\n').enumerate().skip(1) {
            assert_eq!(
                ln.len() - ln.trim_start().len(),
                k,
                "align line {n}: {out:?}"
            );
        }
        let h = cat(text(&lead), hang(i, vsep(&words)));
        let out = pretty(80, &h).unwrap();
        for ln in out.split('\n').skip(1) {
            assert_eq!(
                ln.len() - ln.trim_start().len(),
                k + i as usize,
                "hang: {out:?}"
            );
        }
        cells += 2;
    }
    eprintln!("L4 align/hang: {cells} cells");
}

// ── L5 ──────────────────────────────────────────────────────────────────────

/// L5, fill. [L] `fillSep = fold (</>)` with `x </> y = x <> softline <> y`
/// and `softline = group line`: each separator breaks iff the next item's
/// first line does not fit, decided pairwise at the real column. Items are
/// themselves groups, so L3′(c) (an item's trailing stops at the next
/// separator's decision) is read too.
#[test]
fn l5_fill_pairwise() {
    let m = Mix {
        fill: 0,
        group: 3,
        if_break: 1,
        ..Mix::BASE
    };
    let mut r = Rng(0x15_0005);
    let mut cells = 0;
    while cells < cases() {
        let n = 2 + r.below(6);
        let sep: &'static str = [" ", ", ", ""][r.below(3) as usize];
        let mut gs = Vec::new();
        for j in 0..n {
            if j > 0 {
                gs.push(G::FillSep(sep));
            }
            gs.push(if r.chance(1, 2) {
                G::Text(WORDS[r.below(6) as usize])
            } else {
                G::Group(gen_seq(&mut r, m, 2, 3))
            });
        }
        let gs = match r.below(3) {
            0 => vec![G::Indent(gs)],
            1 => vec![G::Text("xx"), G::Group(gs)],
            _ => gs,
        };
        let ws = widths(&mut r, 4);
        cells += agree(&gs, &ws, "L5");
    }
    eprintln!("L5: {cells} cells, 0 divergences");
}

/// L5 on the oracle: `FmtBuilder::fill_sep(" ")` lowers to exactly [L]'s
/// `softline` (`group line`), so `linear_join` over words equals [L]
/// `fillSep (map text ws)` at every width.
#[test]
fn l5_linear_join_is_leijen_fill_sep() {
    let mut r = Rng(0x15_0006);
    let mut cells = 0;
    while cells < cases() {
        let ws: Vec<&'static str> = (0..1 + r.below(7))
            .map(|_| WORDS[r.below(6) as usize])
            .collect();
        let mut b = FmtBuilder::new();
        b.linear_join(ws.iter().copied(), " ", |b, s| b.text(s));
        let doc = b.finish().unwrap();
        let leijen = fill_sep(&ws.iter().map(|s| text(s)).collect::<Vec<_>>());
        for w in widths(&mut r, 4) {
            assert_eq!(
                render(&doc, p(w)),
                pretty_pprint(w as isize, &leijen).unwrap(),
                "w={w} words={ws:?}"
            );
            cells += 1;
        }
    }
    eprintln!("L5 linear_join: {cells} cells");
}

// ── L6 ──────────────────────────────────────────────────────────────────────

/// L6, ifBreak. [P] `be w k ((i,FlatAlt x _):z) = be w k ((i,x):z)` and
/// `flatten (FlatAlt _ y) = flatten y`: the branch follows the mode of the
/// innermost enclosing group (flat at depth 0, pprint's convention).
#[test]
fn l6_if_break_follows_the_innermost_group() {
    let m = Mix {
        if_break: 5,
        group: 4,
        hard: 1,
        ..Mix::BASE
    };
    run_law(0x16_0006, m, false, "L6");
}

// ── L7 ──────────────────────────────────────────────────────────────────────

/// L7(i): no line exceeds w unless an unbreakable leaf does. Every line op
/// sits in a group, words carry no space, and no break form emits one, so a
/// space after a line's indentation is a flat line op; [W] §4 `fits` admitted
/// its group only if the line up to its newline fit. A line wider than w may
/// therefore hold no such space.
#[test]
fn l7_i_no_overflow_past_a_breakable_point() {
    let m = Mix {
        hard: 1,
        ..Mix::BASE
    };
    let mut r = Rng(0x17_0071);
    let mut cells = 0;
    while cells < cases() {
        let gs = gen_doc(&mut r, m, true);
        let doc = build(&gs);
        for w in widths(&mut r, 4) {
            let out = render(&doc, p(w));
            for ln in out.split('\n') {
                if ln.len() > w {
                    assert!(
                        !ln.trim_start().contains(' '),
                        "L7(i) w={w}: overfull line {ln:?} holds a flat break\nprogram: {gs:?}"
                    );
                }
            }
            cells += 1;
        }
    }
    eprintln!("L7(i): {cells} cells");
}

/// L7(ii): at w = ∞ the output is the `flatten` rendering ([W] §3, the six
/// `flatten` equations) for documents without a hard line.
#[test]
fn l7_ii_infinite_width_is_flatten() {
    let m = Mix::BASE;
    let mut r = Rng(0x17_0072);
    let mut cells = 0;
    while cells < cases() {
        let wrap = r.chance(1, 2);
        let gs = gen_doc(&mut r, m, wrap);
        let doc = build(&gs);
        let flat = pretty_pprint(
            W_INF as isize,
            &flatten(&lower(doc.ops(), p(W_INF)).unwrap()),
        )
        .unwrap();
        assert_eq!(render(&doc, p(W_INF)), flat, "program: {gs:?}");
        cells += 1;
    }
    eprintln!("L7(ii): {cells} cells");
}

/// L7(iii): the output agrees with the oracle at every width tested — here
/// every width 0..=40 for each document, all constructs mixed.
#[test]
fn l7_iii_oracle_at_every_width() {
    let m = Mix {
        hard: 1,
        fill: 2,
        ..Mix::BASE
    };
    let mut r = Rng(0x17_0073);
    let mut cells = 0;
    let all: Vec<usize> = (0..=40).collect();
    while cells < cases() {
        let wrap = r.chance(1, 2);
        let gs = gen_doc(&mut r, m, wrap);
        cells += agree(&gs, &all, "L7(iii)");
    }
    eprintln!("L7(iii): {cells} cells, 0 divergences");
}

// ── L8 ──────────────────────────────────────────────────────────────────────

/// L8, total: no panic on any call sequence. `finish()` either refuses
/// (`Err`) or returns a document the oracle can lower (a well-nested tree:
/// nothing unbalanced is accepted), and rendering an accepted document never
/// panics at any width, 0 and ∞ included.
#[test]
fn l8_total() {
    let mut r = Rng(0x18_0008);
    let mut cells = 0;
    let (mut ok, mut refused) = (0usize, 0usize);
    while cells < cases() {
        let mut b = FmtBuilder::new();
        let mut cps = Vec::new();
        let mut lcps = Vec::new();
        raw_calls(&mut r, &mut b, &mut cps, &mut lcps, 2);
        match b.finish() {
            Err(_) => refused += 1,
            Ok(doc) => {
                ok += 1;
                assert!(
                    lower(doc.ops(), PRINTER).is_ok(),
                    "an accepted document must be well nested"
                );
                for w in [0, 1, 2, 7, 80, W_INF] {
                    let _ = render(&doc, p(w));
                    cells += 1;
                }
            }
        }
    }
    assert!(refused > 0 && ok > 0, "the generator reaches both sides");
    eprintln!("L8: {cells} cells ({ok} accepted, {refused} refused)");
}

fn raw_calls(
    r: &mut Rng,
    b: &mut FmtBuilder<'static>,
    cps: &mut Vec<(usize, usize, u32)>,
    lcps: &mut Vec<(usize, u32)>,
    depth: u32,
) {
    for _ in 0..1 + r.below(10) {
        match r.below(if depth == 0 { 14 } else { 16 }) {
            0 | 1 => b.text(WORDS[r.below(6) as usize]),
            2 => b.softline(),
            3 => b.hardline(),
            4 => b.group_open(),
            5 => b.group_close(),
            6 => b.indent_open(),
            7 => b.indent_close(),
            8 => b.comma_sep(),
            9 => cps.push(b.checkpoint()),
            10 => {
                if let Some(cp) = cps.pop() {
                    b.restore(cp)
                }
            }
            11 => lcps.push(b.light_checkpoint()),
            12 => {
                if let Some(cp) = lcps.pop() {
                    b.light_restore(cp)
                }
            }
            13 => b.f64(r.below(1000) as f64 / 7.0),
            14 => b.wrap(b'[', b']', |b| raw_calls(r, b, cps, lcps, depth - 1)),
            _ => {
                let mut t = Vec::new();
                let mut e = Vec::new();
                let (s1, s2) = (r.next(), r.next());
                b.if_break(
                    |b| raw_calls(&mut Rng(s1), b, &mut t, &mut lcps.clone(), depth - 1),
                    |b| raw_calls(&mut Rng(s2), b, &mut e, &mut Vec::new(), depth - 1),
                );
            }
        }
    }
}
