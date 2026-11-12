//! sk-v27 LW3.PP-3a: the stream format, version 1 (`rust/STREAM-v1.md`).
//!
//! - decode ∘ encode is the identity on 10⁵ generated documents, and
//!   rendering a decoded stream is byte-equal to rendering the document;
//! - the conformance corpus (`rust/stream-v1/corpus/`) is what this codec
//!   and this renderer produce (set `PP_STREAM_WRITE=1` to rewrite it);
//! - the freeze digest (`rust/STREAM-v1.sha256`) holds, so a change to the
//!   op set, the codec, the specification or the corpus fails here until the
//!   format is re-versioned;
//! - the decoder refuses what is not a canonical version-1 stream, and never
//!   panics on any byte string.

use pprint::stream::{StreamError, TAGS, VERSION, decode, encode};
use pprint::{Doc, FmtBuilder, PRINTER, Printer, render};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ── A deterministic generator over every builder call ──────────────────────

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

const WORDS: [&str; 10] = [
    "a", "bb", "ccc", "x;", "(y)", "日本", "e\u{301}", "a😀", "two\nlines", "",
];

/// Emit a random program of every op the builder can make.
fn prog(r: &mut Rng, b: &mut FmtBuilder<'static>, depth: u32) {
    for _ in 0..1 + r.below(6) {
        let top = if depth == 0 { 19 } else { 24 };
        match r.below(top) {
            0 => b.text(WORDS[r.below(WORDS.len() as u64) as usize]),
            1 => b.text_owned(&format!("o{}", r.below(1000))),
            2 => b.quoted_text(WORDS[r.below(5) as usize]),
            3 => b.char(b",;:=+"[r.below(5) as usize]),
            4 => b.i64(r.next() as i64),
            5 => b.u64(r.next()),
            6 => b.i128(((r.next() as i128) << 64) | r.next() as i128),
            7 => b.u128(((r.next() as u128) << 64) | r.next() as u128),
            8 => b.f64((r.below(1 << 20) as f64) / 7.0 - 3.0),
            9 => b.hardline(),
            10 => b.softline(),
            11 => b.break_line(),
            12 => b.comma_sep(),
            13 => b.sep([" | ", " ->", ""][r.below(3) as usize], ["|", "", "->"][r.below(3) as usize]),
            14 => b.fill_sep([" ", ", ", "", "  "][r.below(4) as usize]),
            15 => {
                let s = WORDS[r.below(5) as usize];
                b.line_suffix(|b| {
                    b.text(" //");
                    b.text(s);
                })
            }
            16 => b.line_suffix_boundary(),
            17 => b.break_parent(),
            18 => b.text_inline_ws(" "),
            19 | 20 => b.group(|b| prog(r, b, depth - 1)),
            21 => b.indent(|b| prog(r, b, depth - 1)),
            22 => {
                let (o, c) = [(b'[', b']'), (b'{', b'}'), (b'(', b')')][r.below(3) as usize];
                b.wrap(o, c, |b| prog(r, b, depth - 1))
            }
            _ => {
                let (s1, s2) = (r.next(), r.next());
                b.if_break(
                    |b| prog(&mut Rng(s1), b, depth - 1),
                    |b| prog(&mut Rng(s2), b, depth - 1),
                )
            }
        }
    }
}

fn gen_doc(seed: u64) -> Doc<'static> {
    let mut r = Rng(seed);
    let mut b = FmtBuilder::new();
    let depth = 1 + r.below(4) as u32;
    prog(&mut r, &mut b, depth);
    b.finish().expect("closure-built programs are balanced")
}

// ── decode ∘ encode = id, and the render is byte-equal ─────────────────────

#[test]
fn decode_encode_is_the_identity_and_renders_byte_equal() {
    let n = cases();
    let mut ops = 0usize;
    for k in 0..n as u64 {
        let doc = gen_doc(0x5EED_0000 + k);
        let bytes = encode(&doc);
        let back = decode(&bytes).unwrap_or_else(|e| panic!("doc {k}: {e:?}\n{doc:?}"));
        // Identity: the same ops (by their Debug form, which shows every
        // payload, f64 by its bits through the encoding) and headers.
        assert_eq!(format!("{:?}", back.ops), format!("{:?}", doc.ops), "doc {k}: ops");
        assert_eq!(back.hdrs, doc.hdrs, "doc {k}: headers");
        assert_eq!(encode(&back), bytes, "doc {k}: re-encode");
        for w in [0usize, 7, 40, 80] {
            assert_eq!(render(&back, p(w)), render(&doc, p(w)), "doc {k} at w={w}");
        }
        ops += doc.ops.len();
    }
    eprintln!("STREAM-v1: {n} documents ({ops} ops), decode ∘ encode = id, renders byte-equal");
}

/// Every op of the op set is in the generated corpus (so the identity above
/// reads each tag), and every op has exactly one tag.
#[test]
fn every_op_has_one_tag_and_the_generator_reaches_it() {
    let mut seen = std::collections::BTreeSet::new();
    for k in 0..2_000u64 {
        for op in gen_doc(0x5EED_0000 + k).ops.iter() {
            let name = format!("{op:?}");
            let name = name
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap()
                .to_string();
            seen.insert(name);
        }
    }
    let tags: Vec<&str> = TAGS.iter().map(|(n, _)| *n).collect();
    for (i, (n, t)) in TAGS.iter().enumerate() {
        assert_eq!(*t as usize, i, "tags are dense and in order");
        assert!(seen.contains(*n), "the generator never makes {n}");
    }
    for n in &seen {
        assert!(tags.contains(&n.as_str()), "{n} has no tag");
    }
}

// ── The conformance corpus ─────────────────────────────────────────────────

/// The widths the corpus records, and pprint's default printer otherwise.
const CORPUS_WIDTHS: [usize; 6] = [0, 10, 20, 40, 80, 2_147_483_647];

/// The hand-written cases: one per construct, then the trivia interplay.
fn hand_cases() -> Vec<(&'static str, Doc<'static>)> {
    let mut v = Vec::new();
    let mut add = |name: &'static str, f: &dyn Fn(&mut FmtBuilder<'static>)| {
        let mut b = FmtBuilder::new();
        f(&mut b);
        v.push((name, b.finish().unwrap()));
    };
    add("leaves", &|b| {
        b.text("t");
        b.text_owned("o");
        b.quoted_text("q");
        b.char(b';');
        b.i64(-42);
        b.u64(42);
        b.i128(i128::MIN);
        b.u128(u128::MAX);
        b.f64(1.5);
    });
    add("unicode", &|b| {
        b.group(|b| {
            b.text("日本語");
            b.softline();
            b.text("e\u{301}");
            b.softline();
            b.text("a😀");
        })
    });
    add("multiline-leaf", &|b| {
        b.group(|b| {
            b.text("x");
            b.softline();
            b.text("two\nlines");
            b.softline();
            b.text("y");
        })
    });
    add("group-indent", &|b| {
        b.group(|b| {
            b.text("fn");
            b.indent(|b| {
                b.softline();
                b.text("body");
                b.hardline();
                b.text("more");
            });
        })
    });
    add("wrap-commas", &|b| {
        b.wrap(b'[', b']', |b| b.join(["one", "two", "three", "four"], |b, s| b.text(s)))
    });
    add("sep-breakline", &|b| {
        b.group(|b| {
            b.text("a");
            b.sep(" | ", "|");
            b.text("b");
            b.break_line();
            b.text("c");
        })
    });
    add("if-break", &|b| {
        b.wrap(b'(', b')', |b| {
            b.text("x");
            b.comma_sep();
            b.text("y");
            b.if_break(|b| b.char(b','), |_| {});
        })
    });
    add("fill", &|b| {
        b.linear_join(
            ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta"],
            " ",
            |b, s| b.text(s),
        )
    });
    add("fill-comma", &|b| {
        b.linear_join(["1", "22", "333", "4444", "55555"], ", ", |b, s| b.text(s))
    });
    add("suffix-hardline", &|b| {
        b.group(|b| {
            b.text("a");
            b.line_suffix(|b| b.text(" // c"));
            b.softline();
            b.text("b");
        });
        b.hardline();
        b.text("z");
    });
    add("suffix-boundary", &|b| {
        b.group(|b| {
            b.text("a");
            b.line_suffix(|b| b.text(" /* c */"));
            b.line_suffix_boundary();
            b.text("b");
        })
    });
    add("boundary-idle", &|b| {
        b.group(|b| {
            b.text("a");
            b.line_suffix_boundary();
            b.text("b");
        })
    });
    add("break-parent", &|b| {
        b.group(|b| {
            b.text("a");
            b.softline();
            b.group(|b| {
                b.text("b");
                b.line_suffix(|b| b.text(" // why"));
                b.break_parent();
            });
        })
    });
    add("blank-lines", &|b| {
        b.text("a");
        for _ in 0..4 {
            b.hardline();
        }
        b.text("b");
    });
    add("suffix-at-end", &|b| {
        b.text("a");
        b.line_suffix(|b| b.text(" // last"));
    });
    v
}

fn corpus() -> Vec<(String, Doc<'static>)> {
    let mut v: Vec<(String, Doc<'static>)> = hand_cases()
        .into_iter()
        .map(|(n, d)| (n.to_string(), d))
        .collect();
    for k in 0..48u64 {
        v.push((format!("gen-{k:02}"), gen_doc(0xC0_7905 + k)));
    }
    v
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The corpus as files: `<name>.ppst` per case, and `manifest.json` with the
/// stream's sha256 and the render at each width (printer indent 4, spaces).
fn corpus_files() -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut m = String::from("{\n  \"format\": \"pprint-stream\",\n");
    m.push_str(&format!("  \"version\": {VERSION},\n"));
    m.push_str(&format!(
        "  \"printer\": {{ \"indent\": {}, \"use_tabs\": false }},\n",
        PRINTER.indent
    ));
    m.push_str("  \"cases\": [\n");
    let cs = corpus();
    for (i, (name, doc)) in cs.iter().enumerate() {
        let bytes = encode(doc);
        let sum = hex(&Sha256::digest(&bytes));
        m.push_str(&format!(
            "    {{ \"name\": {}, \"stream\": {}, \"sha256\": \"{sum}\", \"renders\": [\n",
            json_str(name),
            json_str(&format!("{name}.ppst"))
        ));
        for (j, w) in CORPUS_WIDTHS.iter().enumerate() {
            let out = render(doc, p(*w));
            m.push_str(&format!(
                "      {{ \"width\": {w}, \"output\": {} }}{}\n",
                json_str(&out),
                if j + 1 < CORPUS_WIDTHS.len() { "," } else { "" }
            ));
        }
        m.push_str(&format!("    ] }}{}\n", if i + 1 < cs.len() { "," } else { "" }));
        files.push((format!("{name}.ppst"), bytes));
    }
    m.push_str("  ]\n}\n");
    files.push(("manifest.json".to_string(), m.into_bytes()));
    files
}

#[test]
fn the_conformance_corpus_is_current() {
    let dir = root().join("stream-v1/corpus");
    let files = corpus_files();
    if std::env::var("PP_STREAM_WRITE").is_ok() {
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in &files {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .expect("rust/stream-v1/corpus exists")
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    want.sort();
    assert_eq!(on_disk, want, "the corpus holds exactly the cases");
    for (name, bytes) in &files {
        let got = std::fs::read(dir.join(name)).unwrap();
        assert!(&got == bytes, "{name} is stale (PP_STREAM_WRITE=1 rewrites it)");
        if name.ends_with(".ppst") {
            // Each file decodes, and re-encodes to itself.
            let doc = decode(&got).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(encode(&doc), got, "{name}");
        }
    }
}

// ── The freeze ─────────────────────────────────────────────────────────────

/// `STREAM-v1.sha256` lists the sha256 of the specification, the codec, the
/// op set and the corpus manifest; each must hold (`shasum -a 256 -c` reads
/// the same file). A new op changes `src/op.rs` and `src/stream.rs`, so it
/// fails here: version 1 is frozen.
#[test]
fn the_freeze_digest_holds() {
    let rec = std::fs::read_to_string(root().join("STREAM-v1.sha256")).expect("STREAM-v1.sha256");
    let mut paths = Vec::new();
    for ln in rec.lines().filter(|l| !l.trim().is_empty()) {
        let (sum, path) = ln.split_once("  ").expect("`<sha256>  <path>` lines");
        let got = hex(&Sha256::digest(
            std::fs::read(root().join(path)).unwrap_or_else(|_| panic!("{path} is missing")),
        ));
        assert_eq!(got, sum, "{path} differs from the STREAM-v1 freeze");
        paths.push(path.to_string());
    }
    for need in ["STREAM-v1.md", "src/stream.rs", "src/op.rs", "stream-v1/corpus/manifest.json"] {
        assert!(paths.iter().any(|p| p == need), "the freeze covers {need}");
    }
}

/// The tag table in `STREAM-v1.md` is the codec's.
#[test]
fn the_specification_names_the_codecs_tags() {
    let md = std::fs::read_to_string(root().join("STREAM-v1.md")).unwrap();
    let mut rows = Vec::new();
    for ln in md.lines() {
        let cells: Vec<&str> = ln.split('|').map(str::trim).collect();
        if cells.len() > 3
            && let Ok(t) = cells[1].parse::<u8>()
            && cells[2].starts_with('`')
        {
            rows.push((cells[2].trim_matches('`').to_string(), t));
        }
    }
    let want: Vec<(String, u8)> = TAGS.iter().map(|(n, t)| (n.to_string(), *t)).collect();
    assert_eq!(rows, want);
}

// ── Refusals, and no panic on any input ────────────────────────────────────

fn sample() -> Vec<u8> {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("hello");
        b.softline();
        b.line_suffix(|b| b.text(" // x"));
        b.fill_sep(" ");
        b.char(b';');
    });
    encode(&b.finish().unwrap())
}

#[test]
fn the_decoder_refuses_what_is_not_a_canonical_stream() {
    let good = sample();
    assert!(decode(&good).is_ok());
    assert_eq!(decode(b"PPSX\x01\x00").unwrap_err(), StreamError::BadMagic);
    assert_eq!(decode(b"PPST\x02\x00").unwrap_err(), StreamError::Version(2));
    assert_eq!(decode(&good[..good.len() - 1]).unwrap_err(), StreamError::Truncated);
    let mut longer = good.clone();
    longer.push(0);
    assert_eq!(decode(&longer).unwrap_err(), StreamError::NotCanonical);
    // The last header field (flags) altered: the stream's headers are not
    // the ones the ops imply.
    let mut flags = good.clone();
    *flags.last_mut().unwrap() ^= 0x08;
    assert_eq!(decode(&flags).unwrap_err(), StreamError::NotCanonical);
    // An unknown tag, a non-ASCII byte, a tab, an unbalanced stream.
    let one = |op: &[u8], strings: &[&str]| {
        let mut v = b"PPST\x01\x00".to_vec();
        v.push(strings.len() as u8);
        for s in strings {
            v.push(s.len() as u8);
            v.extend_from_slice(s.as_bytes());
        }
        v.push(1);
        v.extend_from_slice(op);
        v.push(0);
        v
    };
    assert_eq!(decode(&one(&[27], &[])).unwrap_err(), StreamError::BadTag { op: 0, tag: 27 });
    assert_eq!(decode(&one(&[3, 0xFF], &[])).unwrap_err(), StreamError::NotAscii { op: 0 });
    assert_eq!(decode(&one(&[0, 0], &["a\tb"])).unwrap_err(), StreamError::Tab { op: 0 });
    assert_eq!(decode(&one(&[0, 1], &["a"])).unwrap_err(), StreamError::BadString { op: 0 });
    assert!(matches!(decode(&one(&[12], &[])).unwrap_err(), StreamError::Build(_)));
    assert!(matches!(decode(&one(&[24, 1], &[])).unwrap_err(), StreamError::Build(_)));
}

/// L8 for the decoder: no byte string makes it panic, and anything it
/// accepts renders without panic. Mutations of valid streams: flipped,
/// inserted and deleted bytes, and truncations.
#[test]
fn the_decoder_never_panics() {
    let mut r = Rng(0xF022);
    let mut accepted = 0usize;
    let n = cases() / 4;
    for k in 0..n as u64 {
        let mut v = encode(&gen_doc(0xF0_0000 + (k % 512)));
        for _ in 0..1 + r.below(3) {
            if v.is_empty() {
                break;
            }
            let at = r.below(v.len() as u64) as usize;
            match r.below(4) {
                0 => v[at] ^= 1 << r.below(8),
                1 => v.insert(at, r.next() as u8),
                2 => {
                    v.remove(at);
                }
                _ => v.truncate(at),
            }
        }
        if let Ok(doc) = decode(&v) {
            accepted += 1;
            for w in [0usize, 13, 80] {
                let _ = render(&doc, p(w));
            }
        }
    }
    eprintln!("decoder: {n} mutated streams, {accepted} accepted, no panic");
}
