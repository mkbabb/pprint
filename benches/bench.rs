#![feature(test)]

extern crate pprint;
extern crate regex;
extern crate test;

use pprint::{pprint, Doc, Pretty, Printer};
use std::collections::HashMap;
use std::fmt::Debug;
use test::Bencher;

#[derive(Pretty, Debug, Clone)]
#[pprint(verbose)]
pub enum HeyEnum<'a> {
    There(&'a str),
    #[pprint(rename = "MyEnum::A")]
    A,
    B(regex::Regex),
}

#[derive(Pretty, Debug, Clone)]
#[pprint(verbose, rename = "Inner")]
pub struct InnerStrumct<'a> {
    x: &'a str,
    y: HeyEnum<'a>,
    z: (usize, usize),
}

#[derive(Pretty, Debug, Clone)]
#[pprint(verbose)]
pub struct Strumct<'a> {
    a: Vec<usize>,
    b: HashMap<String, HeyEnum<'a>>,
    c: InnerStrumct<'a>,
    #[pprint(ignore)]
    no: usize,
}

// Helper function to create a Strumct with a vector of given size
fn create_strumct(vec_size: usize) -> Strumct<'static> {
    let a = (1..=vec_size).collect();
    let mut b = HashMap::new();
    b.insert("hello".to_string(), HeyEnum::There("there"));
    b.insert("a".to_string(), HeyEnum::A);
    b.insert(
        "b".to_string(),
        HeyEnum::B(regex::Regex::new(".*").unwrap()),
    );
    Strumct {
        a,
        b,
        c: InnerStrumct {
            x: "hello",
            y: HeyEnum::There("there"),
            z: (1, 2),
        },
        no: 0,
    }
}

// Benchmark pretty-printing small vector (10 elements)
#[bench]
fn bench_pprint_small_vector(b: &mut Bencher) {
    let printer = Printer::default();
    let s = create_strumct(10);
    b.iter(|| {
        let pprint = printer.pprint(&s);
        test::black_box(pprint);
    });
}

// Benchmark Debug for small vector (10 elements)
#[bench]
fn bench_debug_small_vector(b: &mut Bencher) {
    let s = create_strumct(10);
    b.iter(|| {
        let debug = format!("{:?}", s);
        test::black_box(debug);
    });
}

// Benchmark pretty-printing medium vector (1000 elements)
#[bench]
fn bench_pprint_medium_vector(b: &mut Bencher) {
    let s = create_strumct(1000);
    b.iter(|| {
        let pprint = pprint(&s, None);
        test::black_box(pprint);
    });
}

// Benchmark Debug for medium vector (1000 elements)
#[bench]
fn bench_debug_medium_vector(b: &mut Bencher) {
    let s = create_strumct(1000);
    b.iter(|| {
        let debug = format!("{:?}", s);
        test::black_box(debug);
    });
}

// Benchmark pretty-printing large vector (100,000 elements)
#[bench]
fn bench_pprint_large_vector(b: &mut Bencher) {
    let printer = Printer::default();
    let s = create_strumct(100_000);
    b.iter(|| {
        let pprint = printer.pprint(&s);
        test::black_box(pprint);
    });
}

// Benchmark Debug for large vector (100,000 elements)
#[bench]
fn bench_debug_large_vector(b: &mut Bencher) {
    let s = create_strumct(100_000);
    b.iter(|| {
        let debug = format!("{:?}", s);
        test::black_box(debug);
    });
}
