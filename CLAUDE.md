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
- `rust/src/op.rs` — `FmtOp` enum (24 bytes, Copy): Text, QuotedText, Char, I64, U64, I128Hi/Lo, U128Hi/Lo, F64, GroupStart/End, IndentStart/End, WrapStart/End, Hardline, Softline, BreakLine, Mediumline, CommaSep, Sep, IfBreak
- `rust/src/builder.rs` — `FmtBuilder`: constructs `Vec<FmtOp>` with auto `GroupStart.flat_width` patching
- `rust/src/render.rs` — Linear-scan renderer, `Printer` config struct, `render()`/`render_with_hint()`
- `rust/src/pretty.rs` — `Pretty` trait, `pprint()` entry point, impls for all std types
- `rust/src/utils.rs` — `text_justify` greedy bin-packing for SmartJoin
- `rust/src/dtoa/` — `DigitCount` trait (integer width), `count_u64_digits()`, `udivmod_1e19()`
- `rust/derive/` — `#[derive(Pretty)]` proc macro generates `Pretty<'a>` impls

### Dependencies
- `ryu = "1.0"` — fast f64→string (shortest decimal representation)
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
- `FmtOp` is `Copy`, 24 bytes per variant (8-byte align) — no Box allocations, no pointer chasing
- `GroupStart.flat_width` (u16) pre-computed by `FmtBuilder` during construction — O(1) break decisions
- `WrapStart`/`WrapEnd` — fused group+delimiter+indent+break_line (8 ops → 2 per container)
- `QuotedText` — fused `"` + text + `"` emission (3 ops → 1 per quoted string)
- `CommaSep` — zero-payload fast path for the universal `", "` / `","` separator pattern
- `Sep` — custom separator with inline flat/break byte arrays (max 8+4 bytes)
- Numeric FmtOp variants (I64, U64, F64) rendered inline by `render()` using itoap/ryu
- i128/u128 use wide-op encoding (I128Hi+I128Lo) — keeps FmtOp Copy, avoids 16-byte alignment inflation
- `DigitCount` trait provides integer width for `FmtBuilder` flat_width tracking
- F64 flat_width uses conservative upper bound (24 bytes)
- Render group stack is a `u64` bitfield (64-level depth, zero allocation). Depths >64 conservatively break.
- `SmartJoin` — greedy bin-packing via text_justify, cursor walk (no HashSet)
- `LinearJoin` — inline greedy breaking during build
- `Join` — delegates break decisions to enclosing Group/IfBreak, uses CommaSep
- `Mediumline` — breaks when `current_line_len > max_width / 2`
- Indent emission uses pre-allocated cache (`extend_from_slice` instead of per-byte push)
- Derive macro `#[derive(Pretty)]` generates `Pretty<'a>` impls using wrap/comma_sep/merged field labels
- Derive attributes: `#[pprint(skip)]`, `#[pprint(indent)]`, `#[pprint(verbose)]`, `#[pprint(rename = "...")]`, `#[pprint(getter = "...")]`
- Release builds use `String::from_utf8_unchecked` — all sources produce valid UTF-8

- String Pretty impls emit quoted text (`"hello"`) via fused `QuotedText` op for unambiguous output

## Testing
```bash
cd rust && cargo test    # 65 tests: 26 builder + 14 derive + 12 digit_count + 13 pretty
```

## Benchmarks
```bash
cd rust && cargo bench   # 38 benchmarks: 26 pprint + 12 digit_count
```
- pprint benches: flat_vec (1k/10k), nested, floats, strings, tuples, narrow_40, wide_120
- Build-only and render-only microbenchmarks
- Deep nesting (50-level) and many-groups (1k) stress tests
- SmartJoin benches: flat_1k, nested (vs default Join)
- Each pprint bench has `Debug` and `pretty` crate (Wadler-Lindig) counterparts
- digit_count benches: all integer types (i8–i128, u8–u128, isize, usize)
