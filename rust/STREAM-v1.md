# pprint stream format, version 1 (sk-v27 LW3.PP-3a)

A versioned byte encoding of a finished pprint document, `Doc { ops, hdrs }` (`src/op.rs`, ABI v2 with its LW3 addendum, `ABI-v2.md`), with a string table. It carries every op of that ABI, including the LW3 additions: `AsciiByte` payloads (PP-1a), display-column widths (PP-1b), `FillSep` (ruling S8-B) and the trivia ops `LineSuffix`, `LineSuffixBoundary` and `BreakParent` (PP-2). The TypeScript renderer (LW4.PP-3-TS) reads it, and its gate is byte-identity with the Rust renderer on the conformance corpus below.

The reference implementation is `src/stream.rs` (`pprint::stream::{encode, decode}`); its gate test is `tests/stream_v1.rs`.

## Layout

All multi-byte integers are unsigned LEB128 varints (`varint`), at most 10 bytes for a u64 and 5 for a u32, unless a field says otherwise. A signed integer is zigzag-encoded before its varint (`zigzag(v) = (v << 1) ^ (v >> 63)`).

```text
stream  = magic version strings ops hdrs
magic   = "PPST"                         (4 bytes)
version = u16 little-endian = 1          (2 bytes)
strings = varint n, then n × (varint len, len bytes of UTF-8)
ops     = varint n, then n × op
op      = tag (1 byte), then the payload of that tag
hdrs    = varint n, then n × 7 varints:
          flat_width, trailing, trailing_pending, cut, cut_pending, end_pc, flags
```

The string table holds each distinct string of a `Text`, `OwnedText` or `QuotedText` leaf once, in order of first use. Nothing follows the header table.

## Op tags

| Tag | Op | Payload |
|---|---|---|
| 0 | `Text` | varint string index |
| 1 | `OwnedText` | varint string index |
| 2 | `QuotedText` | varint string index (the string without its quotes) |
| 3 | `Char` | 1 byte, printable ASCII `0x20..=0x7E` |
| 4 | `I64` | zigzag varint |
| 5 | `U64` | varint |
| 6 | `I128Hi` | varint (the high 64 bits); the next op is `I128Lo` |
| 7 | `I128Lo` | varint (the low 64 bits) |
| 8 | `U128Hi` | varint; the next op is `U128Lo` |
| 9 | `U128Lo` | varint |
| 10 | `F64` | 8 bytes, the IEEE-754 bits, little-endian |
| 11 | `GroupStart` | varint header index |
| 12 | `GroupEnd` | — |
| 13 | `IndentStart` | — |
| 14 | `IndentEnd` | — |
| 15 | `WrapStart` | 1 byte open delimiter (printable ASCII), varint header index |
| 16 | `WrapEnd` | 1 byte close delimiter (printable ASCII) |
| 17 | `Hardline` | — |
| 18 | `Softline` | — |
| 19 | `BreakLine` | — |
| 20 | `CommaSep` | — |
| 21 | `Sep` | 1 byte flat length (≤ 8), that many bytes; 1 byte break length (≤ 4), that many bytes (all printable ASCII) |
| 22 | `IfBreak` | varint break-branch op count, varint flat-branch op count |
| 23 | `FillSep` | varint header index, varint spaces |
| 24 | `LineSuffix` | varint content op count |
| 25 | `LineSuffixBoundary` | — |
| 26 | `BreakParent` | — |

The semantics of every op, and of every header field, are `ABI-v2.md`'s (laws L1–L9 of the pprint design, normative semantics Wadler `best` with rest lookahead).

## Canonical form and validity

A document has exactly one encoding. A decoder accepts a byte string only if it is the encoding of the document it describes:

1. the magic is `PPST` and the version is 1;
2. every string is UTF-8, every string index is in the table, and no text leaf holds a tab;
3. every byte field is printable ASCII, and every `Sep` length is within its field;
4. the ops are a document the builder accepts (law L8: balanced groups, indents and wraps, `IfBreak` and `LineSuffix` ranges inside their regions, `LineSuffix` content of leaves only, every wide op paired);
5. the header table equals the one the ops imply: each group's `flat_width` as the builder counts it, and `trailing`, `trailing_pending`, `cut`, `cut_pending`, `end_pc` and `flags` as `finish()` computes them;
6. re-encoding the document gives the same bytes (so the string table is in first-use order and nothing trails).

So a decoded stream renders exactly as the document it encodes, and a renderer may trust its headers. `pprint::stream::decode` checks all six and never panics on any input; `StreamError` names the first failure.

## Conformance corpus

`stream-v1/corpus/`: one `<name>.ppst` per case (15 hand-written cases, one per construct and the trivia interplay, and 48 generated over every builder call), and `manifest.json`, which gives for each case its stream file, the stream's sha256 and the Rust renderer's output at widths 0, 10, 20, 40, 80 and 2,147,483,647 (pprint's clamp of an infinite width), with indent 4 and spaces. A renderer conforms to version 1 when it produces every one of those outputs from the stream. `tests/stream_v1.rs` regenerates the corpus and fails if a file differs (`PP_STREAM_WRITE=1` rewrites it).

## Freeze

Version 1 is frozen at LW3.PP-3a, after PP-2's ops (order §3 PP-3a: "Freeze before PP-2's ops are in" is the must-not). `STREAM-v1.sha256` records the sha256 of this file, `src/stream.rs`, `src/op.rs` and `stream-v1/corpus/manifest.json`; `shasum -a 256 -c rust/STREAM-v1.sha256` and the test `the_freeze_digest_holds` read it. An op added to the op set changes `src/op.rs` and the codec, so the digest fails: a new op is a new tag in a new version (`STREAM-v2.md`), with its own corpus and freeze, never an edit to version 1.
