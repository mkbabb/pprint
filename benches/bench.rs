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

// Helper function to create a Strumct with a vector of given size

fn create_vec(vec_size: usize) -> Vec<usize> {
    (1..=vec_size).collect()
}

use std::fmt::Write as _;
use std::io::Write as _;

// Benchmark pretty-printing medium vector (1000 elements)
#[bench]
fn bench_pprint_medium_vector(b: &mut Bencher) {
    // let s = create_strumct(1000);

    // let s: Vec<_> = create_vec(100).into_iter().map(|x| x as f64).collect();

    let s: Vec<_> = (0..100).map(|x| {
        create_vec(100).into_iter().map(|x| x as f64).collect::<Vec<_>>()
    }).collect();

    b.iter(|| {
        let pprint = pprint(&s, None);
        test::black_box(pprint);

        // use write to collect into a vec:
        // let tmps = s.iter().map(|x| Doc::from(x));
        // let mut out = Vec::new();

        // tmps.into_iter().for_each(|x| {
        //     match &x {
        //         Doc::Bytes(b) => {
        //             out.extend_from_slice(b);
        //         }
        //         _ => {}
        //     };
        // });

        // test::black_box(String::from_utf8(out));
    });
}

// Benchmark Debug for medium vector (1000 elements)
#[bench]
fn bench_debug_medium_vector(b: &mut Bencher) {
    // let s = create_strumct(1000);
    // let s: Vec<_> = create_vec(100).into_iter().map(|x| x as f64).collect();
    let s: Vec<_> = (0..100).map(|x| {
        create_vec(100).into_iter().map(|x| x as f64).collect::<Vec<_>>()
    }).collect();

    b.iter(|| {
        let debug = format!("{:?}", s);
        test::black_box(debug);
    });
}
