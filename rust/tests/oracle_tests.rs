//! sk-v27 LW1.PP-3: the oracle's own equations, checked against the worked
//! examples the sources print, and against `pretty` 0.12 on the shared
//! subset (`text`, `line`, `line_`, `nest`, `group`, `<>`).

mod oracle;

use oracle::lower::{LowerError, oracle_render};
use oracle::*;
use pprint::{FmtBuilder, PRINTER, Printer};

fn texts(ws: &str) -> Vec<D> {
    ws.split_whitespace().map(text).collect()
}

// ── [W] §3 laws, observed through `pretty` ──────────────────────────────────

#[test]
fn w3_flatten_line_is_space() {
    // flatten LINE = TEXT " "
    let d = flatten(&cat(text("a"), cat(line(), text("b"))));
    assert_eq!(pretty(0, &d).unwrap(), "a b");
}

#[test]
fn w3_group_chooses_flat_iff_it_fits() {
    // group x = flatten x :<|> x ; better w k x y = if fits (w-k) x then x else y
    let d = group(cat(text("aa"), cat(line(), text("bb"))));
    assert_eq!(pretty(5, &d).unwrap(), "aa bb");
    assert_eq!(pretty(4, &d).unwrap(), "aa\nbb");
}

#[test]
fn w4_fits_negative_width_is_false() {
    // fits w x | w < 0 = False
    assert!(!fits(-1, Be::best(0, 0, nil())));
    // fits w Nil = True
    assert!(fits(0, Be::best(0, 0, nil())));
}

#[test]
fn w4_rest_lookahead_counts_text_after_the_group() {
    // `better` asks `fits` about `be w k ((i,x):z)`: the rest z is in scope
    // up to the first Line. This is the trailing-content law (L1/L3').
    let d = cat(
        group(cat(text("aaa"), cat(line(), text("bbb")))),
        text("cccc"),
    );
    assert_eq!(pretty(11, &d).unwrap(), "aaa bbbcccc");
    assert_eq!(pretty(10, &d).unwrap(), "aaa\nbbbcccc");
}

/// [W] §3, the paper's own example: `showTree` on the tree
/// `aaa[bbbbb[ccc, dd], eee, ffff[gg, hhh, ii]]`.
#[test]
fn w3_show_tree_paper_example() {
    struct Tree(&'static str, Vec<Tree>);
    // showTree (Node s ts) = group (text s <> nest (length s) (showBracket ts))
    fn show_tree(t: &Tree) -> D {
        group(cat(text(t.0), nest(t.0.len() as isize, show_bracket(&t.1))))
    }
    // showBracket [] = nil
    // showBracket ts = text "[" <> nest 1 (showTrees ts) <> text "]"
    fn show_bracket(ts: &[Tree]) -> D {
        if ts.is_empty() {
            return nil();
        }
        cat(text("["), cat(nest(1, show_trees(ts)), text("]")))
    }
    // showTrees [t] = showTree t
    // showTrees (t:ts) = showTree t <> text "," <> line <> showTrees ts
    fn show_trees(ts: &[Tree]) -> D {
        match ts {
            [t] => show_tree(t),
            [t, rest @ ..] => cat(show_tree(t), cat(text(","), cat(line(), show_trees(rest)))),
            [] => nil(),
        }
    }
    let tree = Tree(
        "aaa",
        vec![
            Tree("bbbbb", vec![Tree("ccc", vec![]), Tree("dd", vec![])]),
            Tree("eee", vec![]),
            Tree(
                "ffff",
                vec![Tree("gg", vec![]), Tree("hhh", vec![]), Tree("ii", vec![])],
            ),
        ],
    );
    assert_eq!(
        pretty(30, &show_tree(&tree)).unwrap(),
        "aaa[bbbbb[ccc, dd],\n    eee,\n    ffff[gg, hhh, ii]]"
    );
    assert_eq!(
        pretty(80, &show_tree(&tree)).unwrap(),
        "aaa[bbbbb[ccc, dd], eee, ffff[gg, hhh, ii]]"
    );
}

#[test]
fn w_fill_words() {
    // fill (x:y:zs) = (flatten x <+> fill (flatten y : zs)) :<|> (x </> fill (y:zs))
    let d = w_fill(&texts("the quick brown fox jumps over"));
    assert_eq!(pretty(15, &d).unwrap(), "the quick brown\nfox jumps over");
    assert_eq!(pretty(9, &d).unwrap(), "the quick\nbrown fox\njumps\nover");
}

// ── [L] combinators, checked against Leijen's documented examples ──────────

#[test]
fn l_align_doc_example() {
    // [L] docs: `text "hi" <+> align (vsep [text "nice", text "world"])`
    let d = space_cat(text("hi"), align(vsep(&texts("nice world"))));
    assert_eq!(pretty(80, &d).unwrap(), "hi nice\n   world");
}

#[test]
fn l_hang_doc_example() {
    // [L] docs: `hang 4 (fillSep (map text (words "the hang combinator
    // indents these words !")))` at page width 20.
    let d = hang(
        4,
        fill_sep(&texts("the hang combinator indents these words !")),
    );
    assert_eq!(
        pretty(20, &d).unwrap(),
        "the hang combinator\n    indents these\n    words !"
    );
}

#[test]
fn l_indent_doc_example() {
    // [L] docs: `indent 4 (fillSep (map text (words "the indent
    // combinator indents these words !")))` at page width 20.
    let d = indent(
        4,
        fill_sep(&texts("the indent combinator indents these words !")),
    );
    assert_eq!(
        pretty(20, &d).unwrap(),
        "    the indent\n    combinator\n    indents these\n    words !"
    );
}

#[test]
fn l_padding_fill_doc_example() {
    // [L] docs: `fill i d` pads d to width i (`types` example, fill 6).
    let row = |n: &str, t: &str| space_cat(l_fill(6, text(n)), space_cat(text("::"), text(t)));
    let d = vsep(&[
        row("empty", "Doc"),
        row("nest", "Int -> Doc -> Doc"),
        row("linebreak", "Doc"),
    ]);
    assert_eq!(
        pretty(80, &d).unwrap(),
        "empty  :: Doc\nnest   :: Int -> Doc -> Doc\nlinebreak :: Doc"
    );
}

#[test]
fn p_hardline_forces_the_group_to_break() {
    // [P] flatten hardline = Fail; fits _ SFail = False. At any width the
    // flat alternative is rejected: this is law L2 (HAS_HARD) on the tree.
    let d = group(cat(
        text("a"),
        cat(hardline(), cat(text("b"), cat(line(), text("c")))),
    ));
    assert_eq!(pretty(20_000, &d).unwrap(), "a\nb\nc");
}

// ── Lowering of v1 FmtOp ────────────────────────────────────────────────────

fn p(w: usize) -> Printer {
    Printer {
        max_width: w,
        ..PRINTER
    }
}

#[test]
fn lower_agrees_with_v1_on_a_wrapped_list_without_trailing() {
    let mut b = FmtBuilder::new();
    b.wrap(b'[', b']', |b| {
        b.join(["alpha", "beta", "gamma"], |b, s| b.text(s));
    });
    let ops = b.finish();
    for w in [10, 19, 20, 21, 80] {
        assert_eq!(
            oracle_render(&ops, p(w)).unwrap(),
            pprint::render(&ops, p(w)),
            "w={w}"
        );
    }
    assert_eq!(oracle_render(&ops, p(80)).unwrap(), "[alpha, beta, gamma]");
    assert_eq!(
        oracle_render(&ops, p(10)).unwrap(),
        "[\n    alpha,\n    beta,\n    gamma\n]"
    );
}

#[test]
fn lower_refuses_mediumline() {
    let mut b = FmtBuilder::new();
    b.text("x");
    b.mediumline();
    assert_eq!(
        oracle_render(&b.finish(), p(80)),
        Err(LowerError::Mediumline)
    );
}

#[test]
fn lower_refuses_unbalanced() {
    let ops = vec![pprint::FmtOp::GroupEnd];
    assert!(matches!(
        oracle_render(&ops, p(80)),
        Err(LowerError::Unbalanced(_))
    ));
}

// ── The oracle against `pretty` 0.12 on the shared subset ──────────────────

/// A deterministic generator (SplitMix64; no shrinking: this is a census, the
/// proptest generators of LW2.PP-3 replace it).
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
}

/// One generated document in both representations, plus its S-expression
/// (for divergence reports).
fn gen_pair(r: &mut Rng, depth: u32) -> (D, pretty::RcDoc<'static, ()>, String) {
    use pretty::RcDoc as R;
    const WORDS: [&str; 6] = ["a", "bb", "ccc", "dddd", "eeeee", "ffffff"];
    let pick = if depth == 0 { r.below(3) } else { r.below(7) };
    match pick {
        0 => {
            let w = WORDS[r.below(6) as usize];
            (text(w), R::text(w), format!("{w:?}"))
        }
        // A line is always followed by a word at the same nest: the shared
        // subset excludes pretty-rs's two documented departures from [W]
        // (see `pretty_rs_departures_from_wadler_are_the_two_known_classes`).
        1 => {
            let w = WORDS[r.below(6) as usize];
            (
                cat(line(), text(w)),
                R::line().append(R::text(w)),
                format!("line {w:?}"),
            )
        }
        2 => {
            let w = WORDS[r.below(6) as usize];
            (
                cat(linebreak(), text(w)),
                R::line_().append(R::text(w)),
                format!("line_ {w:?}"),
            )
        }
        3 => {
            let (a, ra, sa) = gen_pair(r, depth - 1);
            (group(a), ra.group(), format!("(group {sa})"))
        }
        4 => {
            let i = r.below(4) as isize;
            let (a, ra, sa) = gen_pair(r, depth - 1);
            (nest(i, a), ra.nest(i), format!("(nest {i} {sa})"))
        }
        _ => {
            let n = 2 + r.below(3);
            let mut d = nil();
            let mut rd = R::nil();
            let mut sd = String::from("(cat");
            for _ in 0..n {
                let (a, ra, sa) = gen_pair(r, depth - 1);
                d = cat(d, a);
                rd = rd.append(ra);
                sd.push(' ');
                sd.push_str(&sa);
            }
            sd.push(')');
            (d, rd, sd)
        }
    }
}

/// Normalise trailing whitespace per line: `pretty` writes indentation
/// lazily, the oracle's [W] `layout` eagerly (`copy i ' '`).
fn norm(s: &str) -> String {
    s.split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn oracle_matches_pretty_0_12_on_the_shared_subset() {
    let mut r = Rng(0x5eed_0027);
    let mut checked = 0usize;
    let mut diverged = Vec::new();
    for _ in 0..2_000 {
        let depth = 1 + r.below(5) as u32;
        let (d, rd, sd) = gen_pair(&mut r, depth);
        // Both sides wrapped in one outer group, as every real document is.
        let d = group(d);
        let rd = rd.group();
        for w in [0usize, 3, 7, 12, 20, 40] {
            let ours = pretty(w as isize, &d).unwrap();
            let theirs = format!("{}", rd.pretty(w));
            checked += 1;
            if norm(&ours) != norm(&theirs) {
                diverged.push((w, sd.clone(), ours, theirs));
            }
        }
    }
    diverged.sort_by_key(|(_, sd, _, _)| sd.len());
    assert!(
        diverged.is_empty(),
        "{} of {checked} (doc,width) cells diverge; smallest: {:?}",
        diverged.len(),
        diverged.first()
    );
    eprintln!("oracle vs pretty 0.12: {checked} cells, 0 divergences");
}

/// The two classes of `pretty` 0.12.5 output that differ from [W] §4, found by
/// the unrestricted generator (300 of 12,000 cells at seed 0x5eed0027: 226
/// class 1, 74 class 2). Both are deliberate in pretty-rs, so the shared
/// subset above excludes them and this test pins them as a record.
#[test]
fn pretty_rs_departures_from_wadler_are_the_two_known_classes() {
    type R = pretty::RcDoc<'static, ()>;
    // Class 1: a line takes the indentation of the *next command*
    // (pretty-rs render.rs, `Doc::Hardline`: "The next document may have
    // different indentation so we should use it"). [W] §4:
    // `be w k ((i,LINE):z) = i `Line` ...`, the line's own nest.
    let ours = pretty(0, &group(cat(nest(1, line()), text("eeeee")))).unwrap();
    let theirs = format!(
        "{}",
        R::line().nest(1).append(R::text("eeeee")).group().pretty(0)
    );
    assert_eq!((ours.as_str(), theirs.as_str()), ("\n eeeee", "\neeeee"));
    // Class 2: on an already-overfull line, pretty-rs's `fitting` returns
    // true once the commands run out; [W] §4 `fits w x | w < 0 = False` is
    // checked first, so the group breaks.
    let ours = pretty(0, &cat(text("bb"), group(linebreak()))).unwrap();
    let theirs = format!("{}", R::text("bb").append(R::line_().group()).pretty(0));
    assert_eq!((ours.as_str(), theirs.as_str()), ("bb\n", "bb"));
}
