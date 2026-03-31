# `pprint`

A Rust library for pretty printing using a flat-buffer instruction set. Automatically derive
`Pretty` for structs, enums, and primitive types; vector and map types are also
supported by default; very similar to the `derive(Debug)` macro, just prettier and more
configurable.

## Usage

```rust
use pprint::{PRINTER, Pretty, FmtBuilder, render, pprint};

let v = vec![1, 2, 3];
print!("{}", pprint(&v, PRINTER));
// prints: [1, 2, 3]

// Or build formatting ops directly:
let mut b = FmtBuilder::new();
b.group(|b| {
    b.char(b'[');
    b.indent(|b| {
        b.break_line();
        b.i64(1); b.str(", "); b.i64(2); b.str(", "); b.i64(3);
    });
    b.break_line();
    b.char(b']');
});
print!("{}", render(&b.finish(), PRINTER));
```

## Architecture

The formatting pipeline is a **flat-buffer instruction set** (`FmtOp`) with a linear O(n) renderer.
No tree structure, no Box allocations, no pointer chasing.

`FmtOp` is a small, `Copy` enum (~32 bytes). The entire formatting program is a `Vec<FmtOp>` —
sequential ops are implicitly concatenated.

### FmtOp variants

- **Text/Char** — emit borrowed text or a single ASCII byte
- **Numeric** — I64, U64, I128, U128, F64 — rendered inline by the renderer using itoap/Dragonbox
- **GroupStart/GroupEnd** — break-mode decision regions, `flat_width` pre-computed during construction
- **IndentStart/IndentEnd** — indentation regions
- **Hardline/Softline/BreakLine/Mediumline** — line break variants with different break semantics
- **IfBreak** — conditional op emission based on break mode

### The `Pretty` trait

```rust
pub trait Pretty<'a> {
    fn pretty(&'a self, b: &mut FmtBuilder<'a>);
}
```

The `'a` lifetime ties `self` to the `FmtBuilder`, enabling zero-copy `Text(&'a str)` emission
for borrowed string fields. Implementations exist for all primitives, `Vec`, `HashMap`, `HashSet`,
tuples (2–12), `Option`, `Box`, `&T`, `String`, and `&str`.

### The `Printer` configuration

The `Printer` handles rendering a `Vec<FmtOp>` to a string with configurable options:

-   `max_width` — maximum width of each line
-   `indent` — number of spaces for each indentation level
-   `use_tabs` — use tabs instead of spaces for indentation

Entry points:

-   `pprint(&value, printer)` — builds ops via `Pretty` trait, then renders to `String`
-   `render(&ops, printer)` — renders pre-built ops directly

## Derive Macro

The derive macro allows for easy pretty printing of essentially any type. Here's a
trivial example:

```rust
use pprint::{Pretty, pprint, PRINTER};

#[derive(Pretty)]
struct Point {
    x: f64,
    y: f64
}

let point = Point { x: 1.0, y: 2.0 };
print!("{}", pprint(&point, PRINTER)); // prints "{x: 1, y: 2}"
```

`Pretty` supports an additional attribute, `pprint`, which is used to customize an
object's pretty printing definition. The following options are available:

-   skip: bool: Skip this field — don't include it in the output
-   indent: bool: Indent this field — add a newline and indent before and after
-   rename: Option<String>: Rename this field — use the given string as the field name
-   getter: Option<String>: Use the given function to get the value of this field
-   verbose: bool: Verbose output — include field names in output

```rust
#[derive(Pretty)]
#[pprint(verbose)]
struct Point {
    #[pprint(rename = "x-coordinate")]
    x: f64,
    #[pprint(rename = "y-coordinate")]
    y: f64,
    #[pprint(skip)]
    _skip_me: bool,
}

let point = Point { x: 1.0, y: 2.0, _skip_me: true };
print!("{}", pprint(&point, PRINTER));

// prints:
// Point {
//   x-coordinate: 1,
//   y-coordinate: 2
// }
```

Structures can be arbitrarily nested. More involved examples can be found in
the [tests](rust/tests/derive_tests.rs) file.

## `smart_join`

`smart_join`'s implementation is based off the text justification algorithm: [`text_justify`](rust/src/utils.rs)

`text_justify` uses greedy bin-packing to minimize total overflow across lines. O(n) for all inputs.

For more information on the algorithm in particular, see the above's heavily commented source code, or [Lecture No. 20](https://www.youtube.com/watch?v=ENyox7kNKeY) from MIT's 6.006 course, "Introduction to Algorithms".

## Performance

The FmtOp flat-buffer system achieves O(n) single-pass rendering with zero intermediate
allocations beyond the output buffer. Break decisions are O(1) per Group — `flat_width`
is pre-computed by the `FmtBuilder` during construction.

Throughput varies by workload — leaf-heavy documents (integers, strings) are close to
`Debug`, while `smart_join` adds bin-packing overhead for optimal line breaking.

| Benchmark | pprint (ns) | Debug (ns) | vs Debug |
|-----------|-------------|------------|----------|
| flat_vec_1k (ints) | 9,028 | 20,916 | 2.32x faster |
| flat_vec_10k (ints) | 79,420 | 221,285 | 2.79x faster |
| nested_100x100 | 75,908 | 327,368 | 4.31x faster |
| floats_1k | 19,455 | 68,507 | 3.52x faster |
| strings_1k | 13,754 | 44,407 | 3.23x faster |
| tuples_1k | 29,967 | 161,258 | 5.38x faster |

Compared to the previous tree-based Doc system, the FmtOp flat-buffer is **3–9x faster**
across the board. The old system was slower than `Debug` on strings and tuples; the new
system beats `Debug` on every benchmark by 2–5x.

### Render Optimizations

- **Flat-buffer instruction set.** `FmtOp` is a `Copy` enum (24 bytes). The entire
  formatting program is a `Vec<FmtOp>` — linear scan, no pointer chasing, cache-friendly.
- **Fused container ops.** `WrapStart`/`WrapEnd` combine group + delimiter + indent + break
  into a single op (replaces 8 ops per container with 2).
- **Zero-payload `CommaSep`.** The universal `", "` / `","` separator is a dedicated
  variant with no payload — hardcoded bytes, maximum branch-predictor friendliness.
- **Pre-computed `flat_width`.** `FmtBuilder` tracks accumulated width during construction
  and patches `GroupStart.flat_width` at group close. The renderer makes O(1) break
  decisions without any tree traversal or caching.
- **Bitfield group stack.** Break-mode state for nested groups is a `u64` bitfield —
  zero allocation, O(1) push/pop, supports 64 nesting levels.
- **Inline numeric rendering.** Integer and float FmtOp variants are formatted directly
  into the output buffer during rendering using itoap and Dragonbox — no intermediate
  string allocation. i128/u128 use wide-op encoding (two ops) to keep `FmtOp` at 24 bytes.
- **Indent cache.** Indentation emitted via `extend_from_slice` from a pre-allocated buffer
  instead of per-byte push loops.
- **`LinearJoin` variant.** Inline break decisions during render with no
  `text_justify` pre-pass. Best for code formatting where optimal prose
  justification is unnecessary.
- **Sorted cursor in `SmartJoin`.** Break checks use a cursor walk over pre-sorted
  indices — O(1) per item, no HashSet overhead.
- **Release `from_utf8_unchecked`.** All FmtOp sources produce valid UTF-8, so
  release builds skip validation.

In the [gorgeous](https://github.com/mkbabb/gorgeous) CSS formatter, the pprint
render phase achieves ~1,140 MB/s on bootstrap.css (281KB), making it negligible
compared to parsing and op construction.

See the [benches](rust/benches) directory for more information.
