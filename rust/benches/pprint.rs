extern crate pprint;

use divan::{Bencher, black_box};
use pprint::{FmtBuilder, PRINTER, Pretty, Printer, pprint, render};
use pretty::RcDoc;

fn main() {
    divan::main();
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_flat_vec(n: usize) -> Vec<usize> {
    (0..n).collect()
}

fn create_nested_vec(outer: usize, inner: usize) -> Vec<Vec<usize>> {
    (0..outer).map(|_| (0..inner).collect()).collect()
}

fn create_float_vec(n: usize) -> Vec<f64> {
    (0..n).map(|i| (i as f64) * 1.23456789 + 0.001).collect()
}

fn create_string_vec(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("item_{:04}: the quick brown fox jumps over the lazy dog", i))
        .collect()
}

fn create_mixed_tuples(n: usize) -> Vec<(usize, f64)> {
    (0..n)
        .map(|i| (i, i as f64 * std::f64::consts::PI))
        .collect()
}

// ── pprint benchmarks ────────────────────────────────────────────────────────

#[divan::bench]
fn bench_pp_flat_1k(b: Bencher) {
    let data = create_flat_vec(1_000);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_pp_flat_10k(b: Bencher) {
    let data = create_flat_vec(10_000);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_pp_nested(b: Bencher) {
    let data = create_nested_vec(100, 100);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_pp_floats(b: Bencher) {
    let data = create_float_vec(1_000);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_pp_strings(b: Bencher) {
    let data = create_string_vec(1_000);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_pp_tuples(b: Bencher) {
    let data = create_mixed_tuples(1_000);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_pp_narrow_40(b: Bencher) {
    let data = create_nested_vec(100, 100);
    let printer = Printer::new(40, 4, false);
    b.bench(|| black_box(pprint(&data, printer)));
}

#[divan::bench]
fn bench_pp_wide_120(b: Bencher) {
    let data = create_nested_vec(100, 100);
    let printer = Printer::new(120, 4, false);
    b.bench(|| black_box(pprint(&data, printer)));
}

// ── Build-only vs render-only ────────────────────────────────────────────────

#[divan::bench]
fn bench_build_only_1k(b: Bencher) {
    let data = create_flat_vec(1_000);
    b.bench(|| {
        let mut builder = FmtBuilder::new();
        data.pretty(&mut builder);
        black_box(builder.finish());
    });
}

#[divan::bench]
fn bench_render_only_1k(b: Bencher) {
    let data = create_flat_vec(1_000);
    let mut builder = FmtBuilder::new();
    data.pretty(&mut builder);
    let ops = builder.finish();
    b.bench(|| black_box(render(&ops, PRINTER)));
}

// ── Stress benchmarks ────────────────────────────────────────────────────────

#[divan::bench]
fn bench_deep_nesting_50(b: Bencher) {
    // 50-level deep nested vecs
    fn nest(depth: usize) -> Vec<Vec<usize>> {
        if depth == 0 {
            vec![vec![1, 2, 3]]
        } else {
            vec![vec![depth]; depth]
        }
    }
    let data = nest(50);
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

#[divan::bench]
fn bench_many_groups_1k(b: Bencher) {
    // 1000 small tuples → 1000 groups
    let data: Vec<(usize, usize)> = (0..1000).map(|i| (i, i + 1)).collect();
    b.bench(|| black_box(pprint(&data, PRINTER)));
}

// ── SmartJoin benchmarks ─────────────────────────────────────────────────────

#[divan::bench]
fn bench_pp_smart_join_1k(b: Bencher) {
    let data = create_flat_vec(1_000);
    b.bench(|| {
        let mut builder = FmtBuilder::new();
        builder.wrap(b'[', b']', |b| {
            b.smart_join(data.iter(), ", ", 80, 1, |b, item| {
                item.pretty(b);
            });
        });
        let ops = builder.finish();
        black_box(render(&ops, PRINTER))
    });
}

#[divan::bench]
fn bench_pp_smart_join_nested(b: Bencher) {
    let data = create_nested_vec(100, 100);
    b.bench(|| {
        let mut builder = FmtBuilder::new();
        builder.wrap(b'[', b']', |b| {
            b.smart_join(data.iter(), ", ", 80, 1, |b, inner| {
                b.wrap(b'[', b']', |b| {
                    b.smart_join(inner.iter(), ", ", 80, 1, |b, item| {
                        item.pretty(b);
                    });
                });
            });
        });
        let ops = builder.finish();
        black_box(render(&ops, PRINTER))
    });
}

// ── pretty crate benchmarks (Wadler-Lindig doc algebra) ─────────────────────

fn pretty_crate_flat_vec(items: &[usize], width: usize) -> String {
    let docs: Vec<RcDoc<()>> = items.iter().map(|i| RcDoc::as_string(i)).collect();
    let sep = RcDoc::text(",").append(RcDoc::line());
    let body = RcDoc::intersperse(docs, sep);
    let doc = RcDoc::text("[")
        .append(RcDoc::line_().append(body).nest(4).append(RcDoc::line_()))
        .append(RcDoc::text("]"))
        .group();
    let mut w = Vec::new();
    doc.render(width, &mut w).unwrap();
    String::from_utf8(w).unwrap()
}

fn pretty_crate_nested_vec(items: &[Vec<usize>], width: usize) -> String {
    let inner_docs: Vec<RcDoc<()>> = items
        .iter()
        .map(|inner| {
            let docs: Vec<RcDoc<()>> = inner.iter().map(|i| RcDoc::as_string(i)).collect();
            let sep = RcDoc::text(",").append(RcDoc::line());
            let body = RcDoc::intersperse(docs, sep);
            RcDoc::text("[")
                .append(RcDoc::line_().append(body).nest(4).append(RcDoc::line_()))
                .append(RcDoc::text("]"))
                .group()
        })
        .collect();
    let sep = RcDoc::text(",").append(RcDoc::line());
    let body = RcDoc::intersperse(inner_docs, sep);
    let doc = RcDoc::text("[")
        .append(RcDoc::line_().append(body).nest(4).append(RcDoc::line_()))
        .append(RcDoc::text("]"))
        .group();
    let mut w = Vec::new();
    doc.render(width, &mut w).unwrap();
    String::from_utf8(w).unwrap()
}

fn pretty_crate_string_vec(items: &[String], width: usize) -> String {
    let docs: Vec<RcDoc<()>> = items
        .iter()
        .map(|s| RcDoc::text("\"").append(RcDoc::text(s.as_str())).append(RcDoc::text("\"")))
        .collect();
    let sep = RcDoc::text(",").append(RcDoc::line());
    let body = RcDoc::intersperse(docs, sep);
    let doc = RcDoc::text("[")
        .append(RcDoc::line_().append(body).nest(4).append(RcDoc::line_()))
        .append(RcDoc::text("]"))
        .group();
    let mut w = Vec::new();
    doc.render(width, &mut w).unwrap();
    String::from_utf8(w).unwrap()
}

fn pretty_crate_tuples(items: &[(usize, f64)], width: usize) -> String {
    let docs: Vec<RcDoc<()>> = items
        .iter()
        .map(|(a, b)| {
            let inner = RcDoc::as_string(a)
                .append(RcDoc::text(","))
                .append(RcDoc::line())
                .append(RcDoc::as_string(b));
            RcDoc::text("(")
                .append(RcDoc::line_().append(inner).nest(4).append(RcDoc::line_()))
                .append(RcDoc::text(")"))
                .group()
        })
        .collect();
    let sep = RcDoc::text(",").append(RcDoc::line());
    let body = RcDoc::intersperse(docs, sep);
    let doc = RcDoc::text("[")
        .append(RcDoc::line_().append(body).nest(4).append(RcDoc::line_()))
        .append(RcDoc::text("]"))
        .group();
    let mut w = Vec::new();
    doc.render(width, &mut w).unwrap();
    String::from_utf8(w).unwrap()
}

fn pretty_crate_float_vec(items: &[f64], width: usize) -> String {
    let docs: Vec<RcDoc<()>> = items.iter().map(|f| RcDoc::as_string(f)).collect();
    let sep = RcDoc::text(",").append(RcDoc::line());
    let body = RcDoc::intersperse(docs, sep);
    let doc = RcDoc::text("[")
        .append(RcDoc::line_().append(body).nest(4).append(RcDoc::line_()))
        .append(RcDoc::text("]"))
        .group();
    let mut w = Vec::new();
    doc.render(width, &mut w).unwrap();
    String::from_utf8(w).unwrap()
}

#[divan::bench]
fn bench_pretty_flat_1k(b: Bencher) {
    let data = create_flat_vec(1_000);
    b.bench(|| black_box(pretty_crate_flat_vec(&data, 80)));
}

#[divan::bench]
fn bench_pretty_flat_10k(b: Bencher) {
    let data = create_flat_vec(10_000);
    b.bench(|| black_box(pretty_crate_flat_vec(&data, 80)));
}

#[divan::bench]
fn bench_pretty_nested(b: Bencher) {
    let data = create_nested_vec(100, 100);
    b.bench(|| black_box(pretty_crate_nested_vec(&data, 80)));
}

#[divan::bench]
fn bench_pretty_floats(b: Bencher) {
    let data = create_float_vec(1_000);
    b.bench(|| black_box(pretty_crate_float_vec(&data, 80)));
}

#[divan::bench]
fn bench_pretty_strings(b: Bencher) {
    let data = create_string_vec(1_000);
    b.bench(|| black_box(pretty_crate_string_vec(&data, 80)));
}

#[divan::bench]
fn bench_pretty_tuples(b: Bencher) {
    let data = create_mixed_tuples(1_000);
    b.bench(|| black_box(pretty_crate_tuples(&data, 80)));
}

// ── Debug benchmarks — same data, format!("{:#?}") ───────────────────────────

#[divan::bench]
fn bench_debug_flat_1k(b: Bencher) {
    let data = create_flat_vec(1_000);
    b.bench(|| black_box(format!("{:#?}", data)));
}

#[divan::bench]
fn bench_debug_flat_10k(b: Bencher) {
    let data = create_flat_vec(10_000);
    b.bench(|| black_box(format!("{:#?}", data)));
}

#[divan::bench]
fn bench_debug_nested(b: Bencher) {
    let data = create_nested_vec(100, 100);
    b.bench(|| black_box(format!("{:#?}", data)));
}

#[divan::bench]
fn bench_debug_floats(b: Bencher) {
    let data = create_float_vec(1_000);
    b.bench(|| black_box(format!("{:#?}", data)));
}

#[divan::bench]
fn bench_debug_strings(b: Bencher) {
    let data = create_string_vec(1_000);
    b.bench(|| black_box(format!("{:#?}", data)));
}

#[divan::bench]
fn bench_debug_tuples(b: Bencher) {
    let data = create_mixed_tuples(1_000);
    b.bench(|| black_box(format!("{:#?}", data)));
}
