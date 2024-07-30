#![feature(test)]

use pprint::count::DigitCount;
use rand::{distributions::uniform::SampleUniform, Rng};
use std::fmt::Display;

extern crate test;
use test::Bencher;

fn bench_range<T>(b: &mut Bencher, start: T, end: T, n_samples: usize)
where
    T: DigitCount + Display + PartialOrd + Copy + SampleUniform,
    rand::distributions::Standard: rand::distributions::Distribution<T>,
{
    let mut rng = rand::thread_rng();
    let samples: Vec<T> = (0..n_samples).map(|_| rng.gen_range(start..=end)).collect();

    b.iter(|| {
        for sample in &samples {
            test::black_box(sample.len());
        }
    });
}

// Benchmark tests for various integer types
#[bench]
fn bench_i8_range(b: &mut Bencher) {
    bench_range(b, i8::MIN, i8::MAX, 100);
}

#[bench]
fn bench_u8_range(b: &mut Bencher) {
    bench_range(b, u8::MIN, u8::MAX, 100);
}

#[bench]
fn bench_i16_range(b: &mut Bencher) {
    bench_range(b, i16::MIN, i16::MAX, 1000);
}

#[bench]
fn bench_u16_range(b: &mut Bencher) {
    bench_range(b, u16::MIN, u16::MAX, 1000);
}

#[bench]
fn bench_i32_range(b: &mut Bencher) {
    bench_range(b, i32::MIN, i32::MAX, 10000);
}

#[bench]
fn bench_u32_range(b: &mut Bencher) {
    bench_range(b, u32::MIN, u32::MAX, 10000);
}

#[bench]
fn bench_i64_range(b: &mut Bencher) {
    bench_range(b, i64::MIN, i64::MAX, 100000);
}

#[bench]
fn bench_u64_range(b: &mut Bencher) {
    bench_range(b, u64::MIN, u64::MAX, 100000);
}

#[bench]
fn bench_isize_range(b: &mut Bencher) {
    bench_range(b, isize::MIN, isize::MAX, 100000);
}

#[bench]
fn bench_usize_range(b: &mut Bencher) {
    bench_range(b, usize::MIN, usize::MAX, 100000);
}

#[bench]
fn bench_i128_range(b: &mut Bencher) {
    bench_range(b, i128::MIN, i128::MAX, 1000000);
}

#[bench]
fn bench_u128_range(b: &mut Bencher) {
    bench_range(b, u128::MIN, u128::MAX, 1000000);
}
