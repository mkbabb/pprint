#![feature(test)]

extern crate pprint;
extern crate test;

use pprint::{PRINTER, Printer, pprint};
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
fn bench_pprint_flat_vec_1k(b: &mut Bencher) {
    let data = create_flat_vec(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pprint_flat_vec_10k(b: &mut Bencher) {
    let data = create_flat_vec(10_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pprint_nested_100x100(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pprint_floats_1k(b: &mut Bencher) {
    let data = create_float_vec(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pprint_strings_1k(b: &mut Bencher) {
    let data = create_string_vec(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pprint_tuples_1k(b: &mut Bencher) {
    let data = create_mixed_tuples(1_000);
    b.iter(|| test::black_box(pprint(&data, PRINTER)));
}

#[bench]
fn bench_pprint_narrow_40col(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    let printer = Printer {
        max_width: 40,
        ..Default::default()
    };
    b.iter(|| test::black_box(pprint(&data, printer)));
}

#[bench]
fn bench_pprint_wide_120col(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    let printer = Printer {
        max_width: 120,
        ..Default::default()
    };
    b.iter(|| test::black_box(pprint(&data, printer)));
}

// ── Debug benchmarks — same data, format!("{:#?}") ───────────────────────────

#[bench]
fn bench_debug_flat_vec_1k(b: &mut Bencher) {
    let data = create_flat_vec(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_flat_vec_10k(b: &mut Bencher) {
    let data = create_flat_vec(10_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_nested_100x100(b: &mut Bencher) {
    let data = create_nested_vec(100, 100);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_floats_1k(b: &mut Bencher) {
    let data = create_float_vec(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_strings_1k(b: &mut Bencher) {
    let data = create_string_vec(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}

#[bench]
fn bench_debug_tuples_1k(b: &mut Bencher) {
    let data = create_mixed_tuples(1_000);
    b.iter(|| test::black_box(format!("{:#?}", data)));
}
