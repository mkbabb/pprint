# pprint

Flexible and lightweight pretty printing library for Rust.

## Build
cargo build
cargo test
cargo clippy --workspace -- -D warnings

## Structure
src/lib.rs         # crate root, re-exports
src/doc.rs         # Doc IR — Nil, Text, Hardline, Concat, Nest, Group, Join, SmartJoin, etc.
src/print.rs       # Wadler-Lindig printer — fits check, layout engine, dtoa
src/utils.rs       # text_justify DP algorithm for SmartJoin line breaking
src/dtoa/          # fast f64→string (Schubfach algorithm)
derive/            # pprint_derive proc macro (#[derive(Pretty)])

## Conventions
- Edition 2024, rust-version 1.85 (nightly)
- Zero clippy warnings (`-D warnings` in CI)
- `Doc` is the core IR; all formatting builds a `Doc` tree, then `pprint()` renders it
- Derive macro uses `#[pprint(rename = "...")]` and `#[pprint(skip)]` / `#[pprint(ignore)]` attributes
- `Printer` is `Copy`—all fields are `usize`/`bool`
- `Score` in text_justify is `Copy`—eliminates clones in O(n^2) DP loop
- `count_text_length()` is memoized via `HashMap<*const Doc, usize>` in `PrintState`
- `SmartJoin` reuses `doc_lengths: Vec<usize>` across calls (cleared, not reallocated)
- `text_justify` memo buffer pooled in `PrintState`—reused across SmartJoin calls, no per-call allocation
- `space_cache` pre-allocated to 128 bytes; output buffer sized from initial text length estimate
- `Join`/`SmartJoin` use tuple-boxed form `Box<(Doc, Vec<Doc>)>` to reduce Doc enum size
- Group flat-mode check accounts for `current_line_len` (Wadler-Lindig correct)
- `IfBreak` propagates `break_mode` through `PrintItem` stack (no fragile peek heuristic)
- `text_justify` uses `saturating_pow(3)` + line clamping to prevent overflow
- `Bytes`/`SmallBytes` validated via `from_utf8().expect()` (no `unsafe`)

## Testing
cargo test              # 16 tests: 4 derive + 12 digit_count

## Benchmarks
cargo bench             # 26 benchmarks: 14 pprint + 12 digit_count
- pprint benches: flat_vec (1k/10k), nested_100x100, floats_1k, strings_1k, tuples_1k, narrow_40col, wide_120col
- Each pprint bench has a `Debug` counterpart for direct comparison
- digit_count benches: all integer types (i8–i128, u8–u128, isize, usize)
