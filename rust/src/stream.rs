//! The pprint stream format, version 1 (sk-v27 LW3.PP-3a; `rust/STREAM-v1.md`).
//!
//! A versioned byte encoding of a finished [`Doc`]: the op stream, its
//! group-header table and a string table, carrying every op of ABI v2 with
//! its LW3 addendum (`AsciiByte`, `FillSep`, `LineSuffix`,
//! `LineSuffixBoundary`, `BreakParent`). The TypeScript renderer (LW4) reads
//! it; this module is the specification's reference implementation.
//!
//! The encoding is canonical: one [`Doc`] has exactly one encoding, and
//! [`decode`] accepts a byte string only if it is the encoding of the
//! document it describes. It re-derives every group header from the ops
//! (the flat widths as the builder counts them, then `finish()`'s compile
//! step) and re-encodes, so a decoded document is one the builder could have
//! produced, and rendering it never panics (law L8).

use crate::DigitCount;
use crate::layout::{BuildError, compile, f64_width};
use crate::op::{AsciiByte, Doc, FmtOp};
use crate::width;

/// The magic bytes.
pub const MAGIC: [u8; 4] = *b"PPST";

/// The format version this module reads and writes.
pub const VERSION: u16 = 1;

/// The op tags of version 1, frozen. A new op is a new tag in a new version
/// (`STREAM-v1.md`, "Freeze").
pub const TAGS: [(&str, u8); 27] = [
    ("Text", 0),
    ("OwnedText", 1),
    ("QuotedText", 2),
    ("Char", 3),
    ("I64", 4),
    ("U64", 5),
    ("I128Hi", 6),
    ("I128Lo", 7),
    ("U128Hi", 8),
    ("U128Lo", 9),
    ("F64", 10),
    ("GroupStart", 11),
    ("GroupEnd", 12),
    ("IndentStart", 13),
    ("IndentEnd", 14),
    ("WrapStart", 15),
    ("WrapEnd", 16),
    ("Hardline", 17),
    ("Softline", 18),
    ("BreakLine", 19),
    ("CommaSep", 20),
    ("Sep", 21),
    ("IfBreak", 22),
    ("FillSep", 23),
    ("LineSuffix", 24),
    ("LineSuffixBoundary", 25),
    ("BreakParent", 26),
];

/// Why a byte string is not a version-1 stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// The magic bytes are not `PPST`.
    BadMagic,
    /// The version is not 1.
    Version(u16),
    /// The input ends inside a field.
    Truncated,
    /// A varint is longer than its type or overflows it.
    BadVarint,
    /// A string of the table is not UTF-8.
    NotUtf8 { index: u32 },
    /// An op names a string outside the table.
    BadString { op: u32 },
    /// An unknown op tag.
    BadTag { op: u32, tag: u8 },
    /// A byte field holds a byte with no `AsciiByte`.
    NotAscii { op: u32 },
    /// A `Sep` length past its field.
    BadSep { op: u32 },
    /// A text leaf holds a tab (the builder refuses it, LW3.PP-1a).
    Tab { op: u32 },
    /// The ops are not a document the builder accepts.
    Build(BuildError),
    /// The bytes decode, but are not the canonical encoding of the document
    /// they describe (a header, a string-table order or a trailing byte
    /// differs).
    NotCanonical,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pprint stream v1: {self:?}")
    }
}

impl std::error::Error for StreamError {}

// ── Encoding ───────────────────────────────────────────────────────────────

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn tag(name: &str) -> u8 {
    TAGS.iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, t)| t)
        .expect("every op has a tag")
}

/// Encode `doc` as a version-1 stream.
pub fn encode(doc: &Doc<'_>) -> Vec<u8> {
    // The string table, in order of first use.
    let mut strings: Vec<&str> = Vec::new();
    let mut index: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut ids: Vec<u32> = Vec::with_capacity(doc.ops.len());
    for op in &doc.ops {
        let s: Option<&str> = match op {
            FmtOp::Text(s) | FmtOp::QuotedText(s) => Some(s),
            FmtOp::OwnedText(s) => Some(s.as_str()),
            _ => None,
        };
        if let Some(s) = s {
            let id = *index.entry(s).or_insert_with(|| {
                strings.push(s);
                (strings.len() - 1) as u32
            });
            ids.push(id);
        }
    }

    let mut out = Vec::with_capacity(16 + doc.ops.len() * 3);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    put_varint(&mut out, strings.len() as u64);
    for s in &strings {
        put_varint(&mut out, s.len() as u64);
        out.extend_from_slice(s.as_bytes());
    }
    put_varint(&mut out, doc.ops.len() as u64);
    let mut ids = ids.into_iter();
    for op in &doc.ops {
        match op {
            FmtOp::Text(_) => {
                out.push(tag("Text"));
                put_varint(&mut out, ids.next().unwrap() as u64);
            }
            FmtOp::OwnedText(_) => {
                out.push(tag("OwnedText"));
                put_varint(&mut out, ids.next().unwrap() as u64);
            }
            FmtOp::QuotedText(_) => {
                out.push(tag("QuotedText"));
                put_varint(&mut out, ids.next().unwrap() as u64);
            }
            FmtOp::Char(c) => {
                out.push(tag("Char"));
                out.push(c.get());
            }
            FmtOp::I64(v) => {
                out.push(tag("I64"));
                put_varint(&mut out, zigzag(*v));
            }
            FmtOp::U64(v) => {
                out.push(tag("U64"));
                put_varint(&mut out, *v);
            }
            FmtOp::I128Hi(v) => {
                out.push(tag("I128Hi"));
                put_varint(&mut out, *v);
            }
            FmtOp::I128Lo(v) => {
                out.push(tag("I128Lo"));
                put_varint(&mut out, *v);
            }
            FmtOp::U128Hi(v) => {
                out.push(tag("U128Hi"));
                put_varint(&mut out, *v);
            }
            FmtOp::U128Lo(v) => {
                out.push(tag("U128Lo"));
                put_varint(&mut out, *v);
            }
            FmtOp::F64(v) => {
                out.push(tag("F64"));
                out.extend_from_slice(&v.to_bits().to_le_bytes());
            }
            FmtOp::GroupStart { hdr } => {
                out.push(tag("GroupStart"));
                put_varint(&mut out, *hdr as u64);
            }
            FmtOp::GroupEnd => out.push(tag("GroupEnd")),
            FmtOp::IndentStart => out.push(tag("IndentStart")),
            FmtOp::IndentEnd => out.push(tag("IndentEnd")),
            FmtOp::WrapStart { open, hdr } => {
                out.push(tag("WrapStart"));
                out.push(open.get());
                put_varint(&mut out, *hdr as u64);
            }
            FmtOp::WrapEnd { close } => {
                out.push(tag("WrapEnd"));
                out.push(close.get());
            }
            FmtOp::Hardline => out.push(tag("Hardline")),
            FmtOp::Softline => out.push(tag("Softline")),
            FmtOp::BreakLine => out.push(tag("BreakLine")),
            FmtOp::CommaSep => out.push(tag("CommaSep")),
            FmtOp::Sep {
                flat,
                flat_len,
                brk,
                brk_len,
            } => {
                out.push(tag("Sep"));
                out.push(*flat_len);
                out.extend(flat[..*flat_len as usize].iter().map(|b| b.get()));
                out.push(*brk_len);
                out.extend(brk[..*brk_len as usize].iter().map(|b| b.get()));
            }
            FmtOp::IfBreak {
                break_count,
                flat_count,
            } => {
                out.push(tag("IfBreak"));
                put_varint(&mut out, *break_count as u64);
                put_varint(&mut out, *flat_count as u64);
            }
            FmtOp::FillSep { hdr, spaces } => {
                out.push(tag("FillSep"));
                put_varint(&mut out, *hdr as u64);
                put_varint(&mut out, *spaces as u64);
            }
            FmtOp::LineSuffix { len } => {
                out.push(tag("LineSuffix"));
                put_varint(&mut out, *len as u64);
            }
            FmtOp::LineSuffixBoundary => out.push(tag("LineSuffixBoundary")),
            FmtOp::BreakParent => out.push(tag("BreakParent")),
        }
    }
    put_varint(&mut out, doc.hdrs.len() as u64);
    for h in &doc.hdrs {
        for v in [
            h.flat_width,
            h.trailing,
            h.trailing_pending,
            h.cut,
            h.cut_pending,
            h.end_pc,
            h.flags,
        ] {
            put_varint(&mut out, v as u64);
        }
    }
    out
}

// ── Decoding ───────────────────────────────────────────────────────────────

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, StreamError> {
        let v = *self.b.get(self.at).ok_or(StreamError::Truncated)?;
        self.at += 1;
        Ok(v)
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], StreamError> {
        let end = self.at.checked_add(n).ok_or(StreamError::Truncated)?;
        let s = self.b.get(self.at..end).ok_or(StreamError::Truncated)?;
        self.at = end;
        Ok(s)
    }

    fn varint(&mut self) -> Result<u64, StreamError> {
        let mut v: u64 = 0;
        for i in 0..10 {
            let b = self.byte()?;
            let part = (b & 0x7F) as u64;
            if i == 9 && part > 1 {
                return Err(StreamError::BadVarint);
            }
            v |= part << (7 * i);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(StreamError::BadVarint)
    }

    fn u32(&mut self) -> Result<u32, StreamError> {
        u32::try_from(self.varint()?).map_err(|_| StreamError::BadVarint)
    }

    fn count(&mut self) -> Result<usize, StreamError> {
        let n = self.u32()? as usize;
        // Every element takes at least one byte.
        if n > self.b.len().saturating_sub(self.at) {
            return Err(StreamError::Truncated);
        }
        Ok(n)
    }
}

/// Decode a version-1 stream. The result borrows its `Text` and
/// `QuotedText` leaves from `bytes`.
pub fn decode(bytes: &[u8]) -> Result<Doc<'_>, StreamError> {
    let mut r = Reader { b: bytes, at: 0 };
    if r.bytes(4)? != MAGIC {
        return Err(StreamError::BadMagic);
    }
    let v = u16::from_le_bytes([r.byte()?, r.byte()?]);
    if v != VERSION {
        return Err(StreamError::Version(v));
    }
    let n_strings = r.count()?;
    let mut strings: Vec<&str> = Vec::with_capacity(n_strings);
    for i in 0..n_strings {
        let len = r.count()?;
        let s = std::str::from_utf8(r.bytes(len)?)
            .map_err(|_| StreamError::NotUtf8 { index: i as u32 })?;
        strings.push(s);
    }
    let n_ops = r.count()?;
    let mut ops: Vec<FmtOp<'_>> = Vec::with_capacity(n_ops);
    for i in 0..n_ops {
        let op_ix = i as u32;
        let string = |r: &mut Reader<'_>| -> Result<&str, StreamError> {
            let id = r.u32()? as usize;
            let s: &str = strings.get(id).ok_or(StreamError::BadString { op: op_ix })?;
            if s.as_bytes().contains(&b'\t') {
                return Err(StreamError::Tab { op: op_ix });
            }
            Ok(s)
        };
        let ascii = |b: u8| AsciiByte::new(b).ok_or(StreamError::NotAscii { op: op_ix });
        let t = r.byte()?;
        let op = match t {
            0 => FmtOp::Text(string(&mut r)?),
            1 => FmtOp::OwnedText(string(&mut r)?.to_string()),
            2 => FmtOp::QuotedText(string(&mut r)?),
            3 => FmtOp::Char(ascii(r.byte()?)?),
            4 => FmtOp::I64(unzigzag(r.varint()?)),
            5 => FmtOp::U64(r.varint()?),
            6 => FmtOp::I128Hi(r.varint()?),
            7 => FmtOp::I128Lo(r.varint()?),
            8 => FmtOp::U128Hi(r.varint()?),
            9 => FmtOp::U128Lo(r.varint()?),
            10 => {
                let b = r.bytes(8)?;
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                FmtOp::F64(f64::from_bits(u64::from_le_bytes(a)))
            }
            11 => FmtOp::GroupStart { hdr: r.u32()? },
            12 => FmtOp::GroupEnd,
            13 => FmtOp::IndentStart,
            14 => FmtOp::IndentEnd,
            15 => {
                let open = ascii(r.byte()?)?;
                FmtOp::WrapStart {
                    open,
                    hdr: r.u32()?,
                }
            }
            16 => FmtOp::WrapEnd {
                close: ascii(r.byte()?)?,
            },
            17 => FmtOp::Hardline,
            18 => FmtOp::Softline,
            19 => FmtOp::BreakLine,
            20 => FmtOp::CommaSep,
            21 => {
                let flat_len = r.byte()?;
                if flat_len > 8 {
                    return Err(StreamError::BadSep { op: op_ix });
                }
                let mut flat = [AsciiByte::SPACE; 8];
                for (slot, &b) in flat.iter_mut().zip(r.bytes(flat_len as usize)?) {
                    *slot = ascii(b)?;
                }
                let brk_len = r.byte()?;
                if brk_len > 4 {
                    return Err(StreamError::BadSep { op: op_ix });
                }
                let mut brk = [AsciiByte::SPACE; 4];
                for (slot, &b) in brk.iter_mut().zip(r.bytes(brk_len as usize)?) {
                    *slot = ascii(b)?;
                }
                FmtOp::Sep {
                    flat,
                    flat_len,
                    brk,
                    brk_len,
                }
            }
            22 => FmtOp::IfBreak {
                break_count: r.u32()?,
                flat_count: r.u32()?,
            },
            23 => FmtOp::FillSep {
                hdr: r.u32()?,
                spaces: r.u32()?,
            },
            24 => FmtOp::LineSuffix { len: r.u32()? },
            25 => FmtOp::LineSuffixBoundary,
            26 => FmtOp::BreakParent,
            tag => return Err(StreamError::BadTag { op: op_ix, tag }),
        };
        ops.push(op);
    }
    // The header table is read, then re-derived: the stream's headers must
    // be exactly the ones `finish()` computes for these ops.
    let n_hdrs = r.count()?;
    for _ in 0..n_hdrs * 7 {
        r.u32()?;
    }
    if r.at != bytes.len() {
        return Err(StreamError::NotCanonical);
    }
    let widths = flat_widths(&ops);
    let mut wi = widths.into_iter();
    for op in ops.iter_mut() {
        match op {
            FmtOp::GroupStart { hdr } | FmtOp::WrapStart { hdr, .. } => {
                *hdr = wi.next().unwrap_or(0);
            }
            FmtOp::FillSep { hdr, .. } => *hdr = 0,
            _ => {}
        }
    }
    let doc = compile(ops).map_err(StreamError::Build)?;
    if encode(&doc) != bytes {
        return Err(StreamError::NotCanonical);
    }
    Ok(doc)
}

/// Each group's flat width, in `GroupStart`/`WrapStart` order, counted as
/// [`FmtBuilder`](crate::FmtBuilder) counts it while the document is built
/// (u32, saturating): leaves at their display width (a multi-line leaf at
/// its last line), line ops at their flat width, an `IfBreak` by its flat
/// branch, a `LineSuffix` as zero, a `WrapStart` with both delimiters.
fn flat_widths(ops: &[FmtOp<'_>]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    // Open groups: (index into `out`, width outside the group, is a wrap).
    let mut stack: Vec<(usize, u32, bool)> = Vec::new();
    // Open IfBreak break branches as (end, width to restore).
    let mut branches: Vec<(usize, u32)> = Vec::new();
    let mut running: u32 = 0;
    let mut pc = 0usize;
    while pc < ops.len() {
        while let Some(&(end, saved)) = branches.last() {
            if pc < end {
                break;
            }
            branches.pop();
            running = saved;
        }
        let add = |running: &mut u32, w: u64| {
            *running = running.saturating_add(u32::try_from(w).unwrap_or(u32::MAX));
        };
        match &ops[pc] {
            FmtOp::Text(s) => add(&mut running, width::leaf(s).last as u64),
            FmtOp::QuotedText(s) => add(&mut running, width::quoted(s).last as u64),
            FmtOp::OwnedText(s) => add(&mut running, width::leaf(s).last as u64),
            FmtOp::Char(_) => add(&mut running, 1),
            FmtOp::I64(v) => add(&mut running, v.len() as u64),
            FmtOp::U64(v) => add(&mut running, v.len() as u64),
            FmtOp::I128Hi(hi) => {
                if let Some(FmtOp::I128Lo(lo)) = ops.get(pc + 1) {
                    add(
                        &mut running,
                        ((((*hi as i128) << 64) | (*lo as i128)).len()) as u64,
                    );
                }
            }
            FmtOp::U128Hi(hi) => {
                if let Some(FmtOp::U128Lo(lo)) = ops.get(pc + 1) {
                    add(
                        &mut running,
                        ((((*hi as u128) << 64) | (*lo as u128)).len()) as u64,
                    );
                }
            }
            FmtOp::F64(v) => add(&mut running, f64_width(*v) as u64),
            FmtOp::Softline => add(&mut running, 1),
            FmtOp::CommaSep => add(&mut running, 2),
            FmtOp::Sep { flat_len, .. } => add(&mut running, *flat_len as u64),
            FmtOp::FillSep { spaces, .. } => add(&mut running, *spaces as u64),
            FmtOp::GroupStart { .. } | FmtOp::WrapStart { .. } => {
                let wrap = matches!(ops[pc], FmtOp::WrapStart { .. });
                out.push(0);
                stack.push((out.len() - 1, running, wrap));
                running = if wrap { 1 } else { 0 };
            }
            FmtOp::GroupEnd | FmtOp::WrapEnd { .. } => {
                if let Some((i, outer, wrap)) = stack.pop() {
                    let w = if wrap {
                        running.saturating_add(1)
                    } else {
                        running
                    };
                    out[i] = w;
                    running = outer.saturating_add(w);
                }
            }
            FmtOp::IfBreak { break_count, .. } => {
                let end = pc + 1 + *break_count as usize;
                if *break_count > 0 {
                    branches.push((end, running));
                }
            }
            FmtOp::LineSuffix { len } => {
                pc += 1 + *len as usize;
                continue;
            }
            FmtOp::I128Lo(_)
            | FmtOp::U128Lo(_)
            | FmtOp::IndentStart
            | FmtOp::IndentEnd
            | FmtOp::Hardline
            | FmtOp::BreakLine
            | FmtOp::LineSuffixBoundary
            | FmtOp::BreakParent => {}
        }
        pc += 1;
    }
    out
}
