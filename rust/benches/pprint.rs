#![feature(test)]

extern crate pprint;
extern crate test;

use pprint::{FmtBuilder, PRINTER, Pretty, Printer, pprint, render};
use test::Bencher;

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

#[bench]
fn bench_pp_flat_1k(b: &mut Bencher) {
    let data = create_flat_vec(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pp_flat_10k(b: &mut Bencher) {
    let data = create_flat_vec(10_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pp_nested(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pp_floats(b: &mut Bencher) {
    let data = create_float_vec(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pp_strings(b: &mut Bencher) {
    let data = create_string_vec(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pp_tuples(b: &mut Bencher) {
    let data = create_mixed_tuples(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pp_narrow_40(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    let printer = Printer::new(40, 4, false);
    b.iter(|| test::black_box(pprint(&data, printer)));
}

#[bench]
fn bench_pp_wide_120(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    let printer = Printer::new(120, 4, false);
    b.iter(|| test::black_box(pprint(&data, printer)));
}

// ── Build-only vs render-only ────────────────────────────────────────────────

#[bench]
fn bench_build_only_1k(b: &mut Bencher) {
    let data = create_flat_vec(1_000);
    b.iter(|| {
        let mut builder = FmtBuilder::new();
        data.pretty(&mut builder);
        test::black_box(builder.finish());
    });
}

#[bench]
fn bench_render_only_1k(b: &mut Bencher) {
    let data = create_flat_vec(1_000);
    let mut builder = FmtBuilder::new();
    data.pretty(&mut builder);
    let ops = builder.finish();
    b.iter(|| test::black_box(render(&ops, PRINTER)));
}

// ── Stress benchmarks ────────────────────────────────────────────────────────

#[bench]
fn bench_deep_nesting_50(b: &mut Bencher) {
    // 50-level deep nested vecs
    fn nest(depth: usize) -> Vec<Vec<usize>> {
        if depth == 0 {
            vec![vec![1, 2, 3]]
        } else {
            vec![vec![depth]; depth]
        }
    }
    let data = nest(50);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_many_groups_1k(b: &mut Bencher) {
    // 1000 small tuples → 1000 groups
    let data: Vec<(usize, usize)> = (0..1000).map(|i| (i, i + 1)).collect();
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

// ── Debug benchmarks — same data, format!("{:#?}") ───────────────────────────

#[bench]
fn bench_debug_flat_1k(b: &mut Bencher) {
    let data = create_flat_vec(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_flat_10k(b: &mut Bencher) {
    let data = create_flat_vec(10_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_nested(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_floats(b: &mut Bencher) {
    let data = create_float_vec(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_strings(b: &mut Bencher) {
    let data = create_string_vec(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_tuples(b: &mut Bencher) {
    let data = create_mixed_tuples(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}
