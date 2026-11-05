//! sk-v27 LW1.PP-2: born-RED tests. Each one is `#[ignore]`d with its stated
//! reason and its curing unit, so the branch's CI stays green while the
//! defect stays on record. Run them with
//! `cargo test --test born_red_lw1 -- --ignored`: each must FAIL, for the
//! reason in its doc comment, until the curing unit lands; the cure removes
//! the `#[ignore]`.
//!
//! Every expected string is pinned twice: by hand here, and by the Wadler
//! oracle (`tests/oracle/`), whose agreement is asserted first so a RED can
//! never come from a wrong expectation.

mod oracle;

use oracle::lower::oracle_render;
use pprint::{FmtBuilder, FmtOp, PRINTER, Printer, render};

fn p(w: usize) -> Printer {
    Printer {
        max_width: w,
        ..PRINTER
    }
}

/// Assert the oracle reproduces `expected`, then that the v1 renderer does.
fn check(ops: &[FmtOp<'_>], w: usize, expected: &str) {
    let oracle = oracle_render(ops, p(w)).expect("lowerable");
    assert_eq!(
        oracle, expected,
        "oracle disagrees with the pinned expectation (w={w})"
    );
    let got = render(ops, p(w));
    assert_eq!(
        got, expected,
        "v1 renderer diverges from the oracle (w={w})"
    );
}

// ── flat_width u16 overflow (group > 65,535) ────────────────────────────────

/// RED reason: `group_close` stores `flat_width.min(u16::MAX)`, so a group
/// whose flat width is 66,002 is recorded as 65,535 and judged to fit a line
/// of 65,600 columns. The oracle (and B.2 "flat_width is a u32") break it.
///
/// Cure: LW2.PP-1 (ABI v2 `GroupHdr{flat_width: u32}`).
#[test]
#[ignore = "born-RED LW1.PP-2: GroupStart.flat_width saturates at u16::MAX; cured by LW2.PP-1"]
fn red_group_flat_width_u16_saturation() {
    let long = "a".repeat(66_000);
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text(&long);
        b.softline();
        b.text("b");
    });
    let ops = b.finish();
    assert_eq!(
        match ops[0] {
            FmtOp::GroupStart { flat_width } => flat_width,
            _ => unreachable!(),
        },
        u16::MAX,
        "precondition: the header saturated"
    );
    check(&ops, 65_600, &format!("{long}\nb"));
}

/// RED reason: the same saturation in `wrap` (`WrapStart.flat_width: u16`).
///
/// Cure: LW2.PP-1.
#[test]
#[ignore = "born-RED LW1.PP-2: WrapStart.flat_width saturates at u16::MAX; cured by LW2.PP-1"]
fn red_wrap_flat_width_u16_saturation() {
    let long = "a".repeat(66_000);
    let mut b = FmtBuilder::new();
    b.wrap(b'[', b']', |b| {
        b.text(&long);
        b.comma_sep();
        b.text("b");
    });
    let ops = b.finish();
    check(&ops, 65_600, &format!("[\n    {long},\n    b\n]"));
}

/// RED reason (found by this unit, same class): `if_break` stores its branch
/// lengths as `(len) as u16`, which *wraps*: a 65,537-op break branch is
/// recorded as 1, and the renderer then emits flat-branch ops in break mode.
///
/// Cure: LW2.PP-1 (`IfBreak{then: Range<u32>, else: Range<u32>}`).
#[test]
#[ignore = "born-RED LW1.PP-2: IfBreak branch counts wrap at u16; cured by LW2.PP-1"]
fn red_if_break_count_u16_wrap() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.hardline(); // force break mode (v1 sentinel)
        b.if_break(
            |b| {
                for _ in 0..65_537 {
                    b.char(b'x');
                }
            },
            |b| b.char(b'y'),
        );
    });
    let ops = b.finish();
    let got = render(&ops, p(80));
    assert_eq!(got, format!("\n{}", "x".repeat(65_537)));
}

// ── L3′: rest lookahead (B.2) ───────────────────────────────────────────────

/// The base case every L3′ clause refines (PP-U3 gate: "at least one greedy
/// divergence on a trailing-content case").
/// RED reason: v1 decides `col + flat_width(G) <= w` and ignores the text
/// after `GroupEnd`; [W] §4 `better` asks `fits` about the rest up to the
/// first line, so "cccc" counts. Cure: LW2.PP-2 (`trailing` at `finish()`).
#[test]
#[ignore = "born-RED LW1.PP-2: v1 group decision ignores trailing text; cured by LW2.PP-2"]
fn red_l3_trailing_text_after_group() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("aaa");
        b.softline();
        b.text("bbb");
    });
    b.text("cccc");
    let ops = b.finish();
    check(&ops, 11, "aaa bbbcccc"); // fits with its trailing: flat (GREEN today)
    check(&ops, 10, "aaa\nbbbcccc"); // 7 + 4 > 10: must break (RED today)
}

/// L3′(b), the `HAS_HARD` sibling. G is followed by "c" and then a later group
/// H that holds a hard line. H is in break mode, so the rest after G stops at
/// H's first line op: trailing(G) = width("c" ++ "d") = 2.
/// - w = 7: col 0 + 5 + 2 = 7 fits, so G is flat. A static `trailing` that
///   counted H's flat width (or the +10000 sentinel) would break G here
///   (critic C2-O1, "too pessimistic"). GREEN today; it guards the cure.
/// - w = 6: 7 > 6, so G must break. RED today: v1 ignores the trailing "cd".
///
/// Cure: LW2.PP-2 (L3′ clause (b) in the monotone-stack `trailing`).
#[test]
#[ignore = "born-RED LW1.PP-2: L3' (b) HAS_HARD-sibling rest lookahead; cured by LW2.PP-2"]
fn red_l3_prime_has_hard_sibling() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("aa");
        b.softline();
        b.text("bb");
    });
    b.text("c");
    b.group(|b| {
        b.text("d");
        b.softline();
        b.text("e");
        b.hardline();
        b.text("f");
    });
    let ops = b.finish();
    check(&ops, 7, "aa bbcd\ne\nf");
    check(&ops, 6, "aa\nbbcd\ne\nf");
}

/// L3′(c), the fill ancestor. v1 has no fill op: a v1 producer fixes every
/// fill separator at *build* time from the items' flat widths, starting from
/// column 0 (that is `linear_join`'s algorithm). The oracle is [W]'s `fill`,
/// which decides each separator at render time, at the real column.
///
/// Document: `"xx" <> fill [group("aa" line "bb") <> ";", "cc"]`.
/// - At w = 10 the first line would be "xxaa bb; cc" (11 columns), so the
///   separator must break. The item's group then sees trailing ";"
///   (2 + 5 + 1 = 8 <= 10) and stays flat: "xxaa bb;\ncc".
/// - RED reason: the build-time decision measured 6 + 1 + 2 = 9 <= 10 from
///   column 0 and emitted a flat " ", so v1 prints "xxaa bb; cc", 11 > 10.
///   L3′(c) requires the separator's own decision at run time, with the
///   item's trailing stopping at the `FillSep`.
///
/// The v1 stream below is written out op by op with that build-time decision,
/// so the RED isolates the static decision from `linear_join`'s own ordering
/// defect (next test).
///
/// Cure: LW2.PP-2 (fill with per-`FillSep` dynamic trailing).
#[test]
#[ignore = "born-RED LW1.PP-2: L3' (c) fill separator decided statically; cured by LW2.PP-2"]
fn red_l3_prime_fill_ancestor() {
    use oracle::*;
    let item0 = cat(group(cat(text("aa"), cat(line(), text("bb")))), text(";"));
    let doc = cat(text("xx"), w_fill(&[item0, text("cc")]));
    assert_eq!(pretty_pprint(10, &doc).unwrap(), "xxaa bb;\ncc");

    let mut b = FmtBuilder::new();
    b.text("xx");
    b.group(|b| {
        b.text("aa");
        b.softline();
        b.text("bb");
    });
    b.char(b';');
    // build-time fill decision from column 0: 6 + 1 + 2 = 9 <= 10, flat.
    b.str(" ");
    b.text("cc");
    assert_eq!(render(&b.finish(), p(10)), "xxaa bb;\ncc");
}

/// RED reason (found by this unit): `linear_join` emits each separator
/// *after* the item it should precede (it calls `emit_item` before deciding
/// the separator), so `["a","b","c"]` joined by ", " renders "ab, c,"
/// before trailing-whitespace stripping. Cure: LW2.PP-2, where `linear_join`
/// is re-expressed as `fill` over `FillSep` (L5).
#[test]
#[ignore = "born-RED LW1.PP-2: linear_join emits the separator after the item; cured by LW2.PP-2"]
fn red_linear_join_separator_order() {
    let mut b = FmtBuilder::new();
    b.linear_join(["a", "b", "c"], ", ", 80, |b, s| b.text(s));
    assert_eq!(render(&b.finish(), p(80)), "a, b, c");
}

// ── L2: structural hard (PP-U2's falsifier) ─────────────────────────────────

/// RED reason: v1 forces a break by adding 10,000 to the group's flat width;
/// at w = 20,000 the sentinel no longer exceeds w, the group is judged flat,
/// and its softline prints as a space next to a hard newline. L2 makes
/// `HAS_HARD` structural. Cure: LW2.PP-1/PP-2 (sentinel removed).
#[test]
#[ignore = "born-RED LW1.PP-2: +10000 hardline sentinel; cured by LW2.PP-1/PP-2"]
fn red_l2_hardline_sentinel_at_wide_width() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("a");
        b.hardline();
        b.text("b");
        b.softline();
        b.text("c");
    });
    check(&b.finish(), 20_000, "a\nb\nc");
}

// ── L8: total (found by this unit) ──────────────────────────────────────────

/// RED reason: the renderer seeds its indent cache with
/// `vec![ch; 128.min(max_width)]`, which is empty at `max_width = 0`; the
/// first indented newline then reads `indent_cache[0]` and panics. L8: no
/// panic on any sequence the builder accepts.
/// Cure: LW3.PP-1 (the one-loop renderer's cached run of spaces).
#[test]
#[ignore = "born-RED LW1.PP-2: render panics at max_width 0 with an indented newline; cured by LW3.PP-1"]
fn red_l8_width_zero_indented_newline_panics() {
    let mut b = FmtBuilder::new();
    b.wrap(b'[', b']', |b| b.text("a"));
    let ops = b.finish();
    let got = std::panic::catch_unwind(|| render(&ops, p(0)));
    assert_eq!(got.ok().as_deref(), Some("[\n    a\n]"));
}
