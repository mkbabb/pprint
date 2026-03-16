---
title: Overview
order: 20
section: pprint
---

# pprint

pprint is a Wadler-Lindig pretty-printer written in Rust. It provides a document algebra for describing how structured data should be laid out, then renders it to a string given a target line width. The core idea: you describe _what_ the output looks like in both flat and broken forms, and the renderer decides _when_ to break.

## The Doc Algebra

A `Doc` is a tree of layout instructions. The fundamental variants are:

| Variant | Purpose |
|---------|---------|
| `Text` (`Char`, `Bytes`, `String`, ...) | Literal text — emitted verbatim |
| `Concat` | Sequence of docs, printed left to right |
| `Group` | Try to print contents on one line; break if it exceeds the width |
| `Indent` / `Dedent` | Increase or decrease indentation for nested content |
| `Hardline` | Unconditional line break (always breaks) |
| `Softline` | Break only when current line exceeds `max_width` |
| `IfBreak` | Conditional — pick one doc when the enclosing Group breaks, another when it fits |
| `Join` | Interleave a separator between a list of docs |
| `SmartJoin` | Like Join, but uses a text-justification algorithm to decide where to break |
| `LinearJoin` | Like Join, but decides breaks with a single forward scan (no pre-pass) |

## How Groups Work

`Group` is the central layout primitive. When the renderer encounters a Group, it measures the total text width of its contents. If the content plus the current column position fits within `max_width`, everything is printed flat (on one line). If it overflows, the Group enters **break mode**, and any `IfBreak` or `Softline` nodes inside it respond accordingly.

```rust
use pprint::{group, indent, hardline, if_break, Doc};

// A JSON-like array: flat when short, broken when long
let items = vec![Doc::from("1"), Doc::from("2"), Doc::from("3")];
let doc = group(
    Doc::from("[")
        + indent(
            if_break(hardline(), Doc::Null)
                + items.join(if_break(
                    Doc::from(",") + hardline(),
                    Doc::from(", "),
                ))
        )
        + if_break(hardline(), Doc::Null)
        + Doc::from("]")
);
```

When this fits in 80 columns, it renders as `[1, 2, 3]`. When it doesn't, it breaks:

```
[
    1,
    2,
    3
]
```

## The Render Loop

The `pprint()` function takes a `Doc` and a `Printer` configuration:

```rust
use pprint::{pprint, Printer};

let printer = Printer::new(80, 4, false); // 80 cols, 4-space indent, no tabs
let output = pprint(doc, printer);
```

Internally, rendering is stack-based (no recursion) and single-pass. The renderer maintains:

- **`current_line_len`** — how far along the current line we are
- **`indent_delta`** — the accumulated indentation level
- **`break_mode`** — whether the enclosing Group decided to break

Each `Doc` node is popped from the stack and dispatched. Literal nodes emit bytes directly. `Group` measures its children, decides flat-or-break, and pushes its child with the appropriate `break_mode`. `IfBreak` checks `break_mode` and selects one of its two branches.

## Printer Configuration

```rust
pub struct Printer {
    pub max_width: usize,  // Target line width (default: 80)
    pub indent: usize,     // Spaces per indent level (default: 4)
    pub use_tabs: bool,    // Indent with tabs instead of spaces
}
```

The default configuration is available as the `PRINTER` constant:

```rust
use pprint::PRINTER; // max_width: 80, indent: 4, use_tabs: false
```

## Performance

pprint is optimized for throughput in formatter pipelines:

- **Stack-based rendering** — no recursion, no stack overflow on deep trees
- **Literal fast-path** — `Char`, `DoubleChar`, `SmallBytes` variants avoid heap allocation for short strings
- **Cached text length** — `FxHashMap` (pre-allocated, 256 capacity) avoids redundant width calculations
- **`LinearJoin`** — inline break decisions with zero pre-pass overhead
- **`SmartJoin`** — text-justification algorithm for optimal line filling
- **`unsafe` UTF-8 skip** — release builds use `String::from_utf8_unchecked` (all inputs validated at construction)

In the gorgeous formatter pipeline, pprint renders at over 1,140 MB/s on a 281KB CSS file.

## Integration with gorgeous

pprint serves as the rendering backend for [gorgeous](/docs/gorgeous/overview), the grammar-derived formatter. The BBNF codegen pipeline transforms parsed ASTs into `Doc` trees using pprint's algebra, then calls `pprint()` to produce the final formatted output. The `@pretty` directives in BBNF grammars (`group`, `indent`, `sep("...")`, `split("...")`) map directly to pprint's `Group`, `Indent`, `Join`, and `SmartJoin` combinators.
