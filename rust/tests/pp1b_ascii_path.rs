//! sk-v27 LW3.PP-1b: the ASCII fast path and the general path agree on every
//! ASCII input.
//!
//! `width::columns` measures a line (no line feed) through the ASCII fast
//! path (`width::ascii_run`) when the line is all ASCII, and through
//! `unicode-width` otherwise. Here both paths are read on the same input:
//! every ASCII line of length ≤ 3 over all 127 non-line-feed ASCII bytes
//! (2,064,640 lines), then 10⁵ longer random ASCII lines, and `ascii_run`
//! against its definition on mixed input at every split point.

use pprint::width::{ascii_run, columns, columns_general};

#[test]
fn ascii_path_equals_general_path_on_every_short_ascii_line() {
    let alphabet: Vec<u8> = (0u8..0x80).filter(|&b| b != b'\n').collect();
    let mut lines = 0usize;
    let mut check = |bytes: &[u8]| {
        let s = std::str::from_utf8(bytes).unwrap();
        assert_eq!(ascii_run(bytes), bytes.len(), "{bytes:?}");
        assert_eq!(columns(s), columns_general(s), "{bytes:?}");
        assert_eq!(columns(s), bytes.len(), "{bytes:?}");
        lines += 1;
    };
    check(&[]);
    for &a in &alphabet {
        check(&[a]);
        for &b in &alphabet {
            check(&[a, b]);
            for &c in &alphabet {
                check(&[a, b, c]);
            }
        }
    }
    assert_eq!(lines, 1 + 127 + 127 * 127 + 127 * 127 * 127);
    eprintln!("ASCII path = general path on {lines} lines of length <= 3");
}

#[test]
fn ascii_path_equals_general_path_on_long_ascii_lines() {
    let mut x: u64 = 0x1b_00a5;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for _ in 0..100_000 {
        let n = (next() % 300) as usize;
        let bytes: Vec<u8> = (0..n)
            .map(|_| {
                let b = (next() % 127) as u8;
                if b >= b'\n' { b + 1 } else { b }
            })
            .collect();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(columns(s), columns_general(s));
        assert_eq!(columns(s), n);
    }
}

#[test]
fn ascii_run_is_the_ascii_prefix_length() {
    let tails = ["", "\u{e9}", "漢", "😀", "\u{301}"];
    for len in 0..=70 {
        for align in 0..8 {
            for tail in tails {
                let mut s = " ".repeat(align);
                s.push_str(&"a~".repeat(len).chars().take(len).collect::<String>());
                let prefix = s.len();
                s.push_str(tail);
                s.push_str("zz");
                let bytes = &s.as_bytes()[align..];
                let expect = if tail.is_empty() {
                    bytes.len()
                } else {
                    prefix - align
                };
                assert_eq!(ascii_run(bytes), expect, "len={len} align={align} tail={tail:?}");
            }
        }
    }
}
