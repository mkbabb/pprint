# pprint op stream, ABI v2 (sk-v27 LW2.PP-1)

The layout record that S15 freezes. The source of truth is `src/op.rs`. The const asserts listed below make any change to these numbers a compile error.

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
| ~~`Mediumline`~~ | | | | **deleted** (a `column` combinator outside L4; LW2.PP-2) |

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

## Not frozen here (later units)

- `Ascii(AsciiByte)`, and the removal of `from_utf8_unchecked`: PP-U1.
- `Nest`, `Align` and `Hang` ops, and the growable mode stack (no 64-level cliff): LW3.PP-1.
- Display-column width (`width.rs`): LW3.PP-1.
- `LineSuffix`, `LineSuffixBoundary` and `BreakParent`: LW3.PP-2 (U4b).
- The versioned stream encoding: LW3.PP-3.
