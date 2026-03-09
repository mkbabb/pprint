# pprint

Flexible and lightweight pretty printing library for Rust.

## Build
cargo build
cargo test
cargo clippy --workspace -- -D warnings

## Structure
src/lib.rs         # crate root, re-exports
src/doc.rs         # Doc IR — Nil, Text, Hardline, Concat, Nest, Group, Join, SmartJoin, LinearJoin, etc.
src/print.rs       # Wadler-Lindig printer — fits check, layout engine, dtoa
src/utils.rs       # text_justify greedy algorithm for SmartJoin line breaking
src/dtoa/          # fast f64→string (Schubfach algorithm)
derive/            # pprint_derive proc macro (#[derive(Pretty)])

## Dependency Graph

```
pprint_derive       (proc-macro, no external mkbabb deps)
    ↓
pprint              ← pprint_derive
    ↓
parse_that          ← pprint
    ↓
bbnf                ← parse_that, pprint
    ↓
bbnf_derive         ← bbnf, parse_that, pprint
    ↓
gorgeous            ← parse_that, bbnf, bbnf_derive, pprint
```

pprint is the root of the Rust crate graph. All downstream crates depend on it.
Local dev uses `.cargo/config.toml` with `[patch.crates-io]`; Cargo.toml uses crates.io versions only.

## Conventions
- Edition 2024, rust-version 1.85 (nightly)
- Zero clippy warnings (`-D warnings` in CI)
- `Doc` is the core IR; all formatting builds a `Doc` tree, then `pprint()` renders it
- Derive macro uses `#[pprint(rename = "...")]` and `#[pprint(skip)]` / `#[pprint(ignore)]` attributes
- `Printer` is `Copy`—all fields are `usize`/`bool`
- `count_text_length()` is memoized via `FxHashMap<*const Doc, usize>` in `PrintState` (rustc-hash for faster hashing)
- `SmartJoin` reuses `doc_lengths: Vec<usize>` across calls (cleared, not reallocated)
- `SmartJoin` uses reverse-iterating cursor for break checks — O(1) per item, not binary_search O(log n)
- `LinearJoin` — inline break decisions during render, no text_justify pre-pass. Best for code formatting.
- `SmartJoin` — greedy bin-packing via text_justify. Best for prose/text justification.
- `Join` — delegates all break decisions to enclosing Group/IfBreak. No autonomous breaks.
- `text_justify` memo buffer pooled in `PrintState`—reused across SmartJoin calls, no per-call allocation
- `pprint_ref(&Doc, Option<Printer>) -> String` — renders by reference without consuming or cloning
- No full-tree pre-pass — output buffer starts at fixed 1024 capacity, `count_text_length` only called lazily by Group/Join
- `space_cache` pre-allocated to 128 bytes
- `Join`/`SmartJoin`/`LinearJoin` use tuple-boxed form `Box<(Doc, Vec<Doc>)>` to reduce Doc enum size
- Release builds use `String::from_utf8_unchecked` — all Doc sources produce valid UTF-8
- Group flat-mode check accounts for `current_line_len` (Wadler-Lindig correct)
- `IfBreak` propagates `break_mode` through `PrintItem` stack (no fragile peek heuristic)
- `text_justify` uses `saturating_pow(3)` + line clamping to prevent overflow
- `Bytes`/`SmallBytes` validated via `from_utf8().expect()` (no `unsafe`)
- `handle_literal` coalesces consecutive spaces: skips `Doc::Char(b' ')` and single-space `Doc::String` when output already ends with whitespace; prevents opaque-span trailing whitespace from doubling with separators

## Testing
cargo test              # 16 tests: 4 derive + 12 digit_count

## Benchmarks
cargo bench             # 26 benchmarks: 14 pprint + 12 digit_count
- pprint benches: flat_vec (1k/10k), nested_100x100, floats_1k, strings_1k, tuples_1k, narrow_40col, wide_120col
- Each pprint bench has a `Debug` counterpart for direct comparison
- digit_count benches: all integer types (i8–i128, u8–u128, isize, usize)
