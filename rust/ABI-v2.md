# pprint op stream, ABI v2 (sk-v27 LW2.PP-1)

The layout record that S15 freezes. ABI v2 breaks the 0.3 public API (`finish() -> Result<Doc, BuildError>`, `render(&Doc, …)`, `mediumline` removed), so it is the 0.4 version line: the crate is `0.4.0-alpha.0` from this ABI on, and a consumer edge onto it says `version = "0.4.0-alpha.0"` (S14 ruling on S13-G2). The source of truth is `src/op.rs`. The const asserts listed below make any change to these numbers a compile error.

## `FmtOp<'a>`: 24 bytes, alignment 8

`const _: () = assert!(size_of::<FmtOp<'static>>() <= 24);` and `align_of == 8`, in `src/op.rs`.

The widest payload is `OwnedText(String)` at 24 bytes. The enum still fits in 24 because rustc stores the discriminant in the niche of `String`'s capacity field, whose high bit is never set. The size assert guards that niche. The 32-byte mutant `Sep { flat: [u8; 24], .. }` fails to compile (evidence `LW2.PP/pp1-mutant-*.log`).

| Variant | Payload | Bytes | Law | Change from v1 |
|---|---|---|---|---|
| `Text(&'a str)` | borrowed slice | 16 | leaf | kept (C2-O5) |
| `OwnedText(String)` | owned | 24 | leaf | — |
| `QuotedText(&'a str)` | borrowed slice | 16 | leaf | — |
| `Char(u8)` | byte | 1 | leaf | — (`AsciiByte` is PP-U1) |
| `I64(i64)`, `U64(u64)` | | 8 | leaf | — |
| `I128Hi/Lo`, `U128Hi/Lo` `(u64)` | wide pair | 8 + 8 | leaf | the pair is now validated by `finish()` |
| `F64(f64)` | | 8 | leaf | the flat width is the exact ryu length, not 24 |
| `GroupStart { hdr: u32 }` | header index | 4 | L1, L2, L3′ | was `{ flat_width: u16 }` |
| `GroupEnd` | — | 0 | | — |
| `IndentStart`, `IndentEnd` | — | 0 | L4 | — |
| `WrapStart { open: u8, hdr: u32 }` | byte + header index | 5 | L1, L4 | was `{ open: u8, flat_width: u16 }` |
| `WrapEnd { close: u8 }` | byte | 1 | | — |
| `Hardline` | — | 0 | L2 | no flat width; the v1 `+10000` sentinel is gone |
| `Softline`, `BreakLine`, `CommaSep` | — | 0 | L1 | — |
| `Sep { flat: [u8; 8], flat_len: u8, brk: [u8; 4], brk_len: u8 }` | | 14 | L1 | — |
| `IfBreak { break_count: u32, flat_count: u32 }` | branch op counts | 8 | L6 | were u16; v1 wrapped at 65,536 |

v1's column-reading line op (it broke when `col > max_width / 2`) is deleted. It was a [L] `column` combinator, which L4 keeps out of the instruction set (LW2.PP-2).

## `GroupHdr`: 16 bytes, one per `GroupStart`/`WrapStart`, in op order

`const _: () = assert!(size_of::<GroupHdr>() == 16);`

| Field | Type | Meaning |
|---|---|---|
| `flat_width` | u32 | width of the group's flattening ([W] §3 `flatten`), saturating; not used when `HAS_HARD` is set |
| `trailing` | u32 | width of the rest after the group, up to its first newline, as [W] §4 `fits` reads it (L3′); saturating |
| `end_pc` | u32 | index of the matching `GroupEnd` or `WrapEnd` |
| `flags` | u32 | bit 0 `HAS_HARD` (the flattening contains a hard line, so the group never fits, L2); bit 1 `WRAP` |

## `Doc<'a> { ops: Vec<FmtOp<'a>>, hdrs: Vec<GroupHdr> }`

`FmtBuilder::finish()` returns `Result<Doc<'a>, BuildError>`. An unbalanced stream is an `Err` (L8). The stream holds at most `u32::MAX - 1` ops. `render(&Doc, Printer)` clamps `max_width` to `u32::MAX / 2`, so `col + flat_width + trailing` cannot overflow.

The renderer's group decision is O(1): a group nested in a flat group is flat. Otherwise the group breaks iff it is `HAS_HARD` or `col + flat_width + trailing > max_width`. `src/layout.rs` derives this rule from [W] §4 and shows it equals `better`/`fits` with the rest lookahead.

## Freeze status (sk-v27 LW2, S15, LG-2.3)

FROZEN in pprint at the 0.4 version line (`0.4.0-alpha.0`): the const asserts above and this record. `ABI-v2.sha256` fixes the bytes of `src/op.rs` and of this file, so any change to either re-takes that record and adds a dated addendum here.

PROVISIONAL for consumers (delegate ruling A-8). bbnf-lang and parse-that still pin pprint `fe7e9ca` (0.3.6), so no consumer compiles against v2 yet. The consumer half closes at PP-U9, when both re-pin onto the 0.4 line with `cargo check --workspace` and `regen --check` GREEN ×2. Any change that port forces is an addendum to this file, never a silent edit.

## Not frozen here (later units)

- `Ascii(AsciiByte)`, and the removal of `from_utf8_unchecked`: PP-U1.
- `Nest`, `Align` and `Hang` ops, and the growable mode stack (no 64-level cliff): LW3.PP-1.
- Display-column width (`width.rs`): LW3.PP-1.
- `LineSuffix`, `LineSuffixBoundary` and `BreakParent`: LW3.PP-2 (U4b).
- The versioned stream encoding: LW3.PP-3.

## Addendum, 2026-11-11: the version line (LW2.TOPO-4d, RULINGS 2026-11-11b TOPO-4-A2)

The layout is unchanged: `src/op.rs` is byte-identical and its sha256 in `ABI-v2.sha256` is the same. Only the version line this record names moves. TOPO-4-A2 publishes `pprint` 0.4.0 from the 0.3-API tree the consumers pin (pprint `lw2/topo-4d-0.4` `6011442`, the content of `fe7e9ca`), because that tree breaks the API of 0.3.6. ABI v2 breaks the API of that 0.4.0 (the five items S13-G2 named: `finish`, `mediumline`, `linear_join`, `render`, `render_with_hint`), so under the same ruling its line is the next minor after 0.4.0: the crate is `0.5.0-alpha.0` from this addendum on, and a consumer edge onto ABI v2 says `version = "0.5.0-alpha.0"`. `0.4.0-alpha.0` is retired unpublished. Where the text above says "the 0.4 version line" or `0.4.0-alpha.0` for ABI v2, read the 0.5 line and `0.5.0-alpha.0`; PP-U9 re-pins the consumers onto the 0.5 line. `ABI-v2.sha256` is re-taken for this file.

## Addendum, 2026-11-12: `AsciiByte` (LW3.PP-1a, under ruling A-8)

The byte fields of the op stream take an ASCII-only type. Every other layout fact above stands, and the size asserts are kept: `FmtOp<'a>` is still ≤ 24 bytes with alignment 8, and `GroupHdr` is still 16 bytes.

`AsciiByte` (`src/op.rs`) is a `#[repr(transparent)]` newtype over `u8` whose constructor `AsciiByte::new(b) -> Option<AsciiByte>` admits only printable ASCII, `0x20..=0x7E` (`const _: () = assert!(size_of::<AsciiByte>() == 1)`). Such a byte is valid UTF-8 on its own and is exactly one display column. It replaces `u8` in four places:

| Variant | v2 | v2 + LW3 addendum | Bytes |
|---|---|---|---|
| `Char` | `Char(u8)` | `Char(AsciiByte)` | 1 |
| `WrapStart` | `{ open: u8, hdr: u32 }` | `{ open: AsciiByte, hdr: u32 }` | 5 |
| `WrapEnd` | `{ close: u8 }` | `{ close: AsciiByte }` | 1 |
| `Sep` | `{ flat: [u8; 8], flat_len: u8, brk: [u8; 4], brk_len: u8 }` | `{ flat: [AsciiByte; 8], flat_len: u8, brk: [AsciiByte; 4], brk_len: u8 }` | 14 |

The builder keeps its byte-taking signatures (`char(u8)`, `wrap(u8, u8, f)`, `sep(&str, &str)`). A byte with no `AsciiByte` (a non-ASCII byte, a tab, a line feed or another control byte) is refused at build: `finish()` returns `BuildError::NotAscii { pc, byte }`. A tab in a text leaf (`text`, `str`, `text_owned`, `quoted_text`, `text_inline_ws`) returns `BuildError::Tab { pc }`, since its display width depends on tab stops the renderer does not have. A `sep` longer than its op holds returns `BuildError::SepTooLong { pc }` (it panicked in release before). These are three new `BuildError` variants.

With every byte typed, the renderer builds its output `String` from `&str` and `AsciiByte` pushes only. `from_utf8_unchecked` is deleted, and `src/lib.rs` carries `#![forbid(unsafe_code)]`, so the crate has no unsafe code and cannot gain any without a compile error.

The renderer is one loop: an `IfBreak` pushes (branch end, resume) on an explicit stack instead of recursing, and the group mode stack is a growable bit vector, so a group past depth 64 reads its true mode (v2's u64 stack read every such group as broken). Neither changes the layout.

`ABI-v2.sha256` is re-taken for `src/op.rs` and this file.
