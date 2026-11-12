//! Display width (sk-v27 LW3.PP-1b).
//!
//! Every width pprint computes (a group's flat width, its trailing rest, the
//! renderer's column) is in display columns, as UAX #11 and the emoji rules
//! of [`unicode_width`] define them: East Asian Wide and Fullwidth characters
//! are 2 columns, combining and other zero-width marks are 0, an emoji
//! presentation, modifier or ZWJ sequence is 2. Bytes are never counted.
//!
//! A leaf is measured line by line. A leaf that holds a line feed is
//! *multi-line*: its first line ends the rest a preceding group reads ([W]
//! §4 `fits w (i `Line` x) = True`), the column after it is the width of its
//! last line, and it sets `HAS_HARD` on its group, since its flattening holds
//! a hard line. Its newline starts the next line at column 0, with no
//! indentation added.
//!
//! ASCII is measured on a fast path: an all-ASCII line is as many columns as
//! it has bytes. [`ascii_run`] is that path's one function (LW3.PP-1c routes
//! it to the `parse-that-simd` all-ASCII kernel).

use unicode_width::UnicodeWidthStr;

/// The length of the leading run of ASCII bytes (`< 0x80`) in `bytes`.
///
/// The ASCII fast path's one function. This is the portable form: the
/// all-ASCII case, which is the common one, is `<[u8]>::is_ascii` (the
/// standard library's word-at-a-time scan); otherwise the run ends at the
/// first byte `>= 0x80`. LW3.PP-1c routes this function to the
/// `parse-that-simd` all-ASCII kernel.
#[inline]
pub fn ascii_run(bytes: &[u8]) -> usize {
    if bytes.is_ascii() {
        bytes.len()
    } else {
        bytes.iter().position(|&b| b >= 0x80).unwrap_or(bytes.len())
    }
}

/// The display columns of one line (a string with no line feed), through the
/// ASCII fast path when the whole line is ASCII.
#[inline]
pub fn columns(line: &str) -> usize {
    let b = line.as_bytes();
    if ascii_run(b) == b.len() {
        b.len()
    } else {
        columns_general(line)
    }
}

/// The display columns of one line through the general path only
/// (`unicode-width`, outside an East Asian context). On every ASCII line it
/// equals [`columns`] (`tests/pp1b_ascii_path.rs`).
#[inline]
pub fn columns_general(line: &str) -> usize {
    UnicodeWidthStr::width(line)
}

/// The width of a text leaf, line by line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeafWidth {
    /// Columns of the first line (the whole leaf when it has one line).
    pub first: usize,
    /// Columns of the last line: the column after the leaf, counted from the
    /// start of that line.
    pub last: usize,
    /// The leaf holds a line feed.
    pub multiline: bool,
}

/// Measure a text leaf: its first and last line, and whether it has more
/// than one.
#[inline]
pub fn leaf(s: &str) -> LeafWidth {
    let b = s.as_bytes();
    match b.iter().position(|&c| c == b'\n') {
        None => {
            let w = columns(s);
            LeafWidth {
                first: w,
                last: w,
                multiline: false,
            }
        }
        Some(i) => {
            let j = b.iter().rposition(|&c| c == b'\n').unwrap_or(i);
            LeafWidth {
                first: columns(&s[..i]),
                last: columns(&s[j + 1..]),
                multiline: true,
            }
        }
    }
}

/// [`leaf`] for a leaf printed between double quotes (`QuotedText`).
#[inline]
pub fn quoted(s: &str) -> LeafWidth {
    let w = leaf(s);
    if w.multiline {
        LeafWidth {
            first: w.first + 1,
            last: w.last + 1,
            multiline: true,
        }
    } else {
        LeafWidth {
            first: w.first + 2,
            last: w.last + 2,
            multiline: false,
        }
    }
}
