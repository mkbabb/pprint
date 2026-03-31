# pprint

Flexible and lightweight pretty printing library — Rust + TypeScript monorepo.

## Structure

```
rust/                      Rust workspace
  Cargo.toml
  src/                     lib.rs, op.rs, builder.rs, render.rs, pretty.rs, utils.rs, dtoa/
  derive/                  pprint_derive proc macro (#[derive(Pretty)])
  benches/
  tests/
typescript/                TS library
  package.json
  src/                     doc.ts, print.ts, utils.ts — port of Rust Doc/Printer
  test/
```

## Build

```bash
cd rust
cargo build
cargo test
cargo clippy --workspace -- -D warnings
```

## Rust Architecture

The formatting pipeline is a **flat-buffer instruction set** (FmtOp) with a linear O(n) renderer.

### Core modules
- `rust/src/op.rs` — `FmtOp` enum: Text, Char, I64, U64, I128, U128, F64, GroupStart/End, IndentStart/End, Hardline, Softline, BreakLine, Mediumline, IfBreak
- `rust/src/builder.rs` — `FmtBuilder`: constructs `Vec<FmtOp>` with auto `GroupStart.flat_width` patching
- `rust/src/render.rs` — Linear-scan renderer, `Printer` config struct, `render()`/`render_with_hint()`
- `rust/src/pretty.rs` — `Pretty` trait, `pprint()` entry point, impls for all std types
- `rust/src/utils.rs` — `text_justify` greedy bin-packing for SmartJoin
- `rust/src/dtoa/` — `DigitCount` trait (integer width), `count_u64_digits()`, `udivmod_1e19()`
- `rust/derive/` — `#[derive(Pretty)]` proc macro generates `Pretty<'a>` impls

### Dependencies
- `dragonbox = "0.1.8"` — fast f64→string (Dragonbox algorithm)
- `itoap = "1.0.1"` — fast integer→string
- `regex` — optional, behind `features = ["regex"]`

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
- `FmtOp` is the core instruction set; all formatting builds a `Vec<FmtOp>`, then `render()` executes it
- `Pretty<'a>` trait: `fn pretty(&'a self, b: &mut FmtBuilder<'a>)` — zero-copy text emission via lifetime tie
- `pprint(&value, PRINTER)` — main entry point, builds ops then renders
- `Printer` is `Copy` — `max_width: usize`, `indent: usize`, `use_tabs: bool`
- `PRINTER` constant: 80 cols, 4 spaces, no tabs
- `FmtOp` is `Copy`, ~24 bytes per variant — no Box allocations, no pointer chasing
- `GroupStart.flat_width` pre-computed by `FmtBuilder` during construction — O(1) break decisions
- Numeric FmtOp variants (I64, U64, I128, U128, F64) rendered inline by `render()` using itoap/dragonbox
- `DigitCount` trait provides integer width for `FmtBuilder` flat_width tracking
- F64 flat_width uses conservative upper bound (24 bytes)
- `SmartJoin` — greedy bin-packing via text_justify, cursor walk (no HashSet)
- `LinearJoin` — inline greedy breaking during build
- `Join` — delegates break decisions to enclosing Group/IfBreak
- `Mediumline` — breaks when `current_line_len > max_width / 2`
- Derive macro `#[derive(Pretty)]` generates `Pretty<'a>` impls with FmtBuilder calls
- Derive attributes: `#[pprint(skip)]`, `#[pprint(indent)]`, `#[pprint(verbose)]`, `#[pprint(rename = "...")]`, `#[pprint(getter = "...")]`
- Release builds use `String::from_utf8_unchecked` — all sources produce valid UTF-8

## Testing
```bash
cd rust && cargo test    # 61 tests: 22 builder + 14 derive + 12 digit_count + 13 pretty
```

## Benchmarks
```bash
cd rust && cargo bench   # 28 benchmarks: 16 pprint + 12 digit_count
```
- pprint benches: flat_vec (1k/10k), nested, floats, strings, tuples, narrow_40, wide_120
- Build-only and render-only microbenchmarks
- Deep nesting (50-level) and many-groups (1k) stress tests
- Each pprint bench has a `Debug` counterpart for direct comparison
- digit_count benches: all integer types (i8–i128, u8–u128, isize, usize)
