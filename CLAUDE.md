# pprint

Flexible and lightweight pretty printing library for Rust.

## Build
cargo build
cargo test
cargo clippy --workspace -- -D warnings

## Structure
src/lib.rs         # crate root, re-exports
src/doc.rs         # Doc IR — Nil, Text, Hardline, Concat, Nest, Group, etc.
src/print.rs       # Wadler-Lindig printer — fits check, layout engine, dtoa
src/utils.rs       # helper traits (IntoPretty, join, wrap, indent)
src/dtoa/          # fast f64→string (Schubfach algorithm)
derive/            # pprint_derive proc macro (#[derive(Pretty)])

## Conventions
- Edition 2024, rust-version 1.85 (nightly)
- Zero clippy warnings (`-D warnings` in CI)
- `Doc` is the core IR; all formatting builds a `Doc` tree, then `print()` renders it
- Derive macro uses `#[pprint(rename = "...")]` and `#[pprint(skip)]` attributes

## Testing
cargo test              # unit + derive tests
cargo bench             # pprint + digit_count benchmarks
