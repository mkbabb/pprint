# `pprint`

A Rust library for pretty printing using a document model. Automatically derive
`Pretty` for structs, enums, and primitive types; vector and map types are also
supported by default; very similar to the `derive(Debug)` macro, just prettier and more
configurable.

## Usage

```rust
use pprint::{Doc, PRINTER, pprint, join, wrap};

let items: Vec<Doc> = vec![1, 2, 3].into_iter().map(Doc::from).collect();
let doc = wrap("[", "]", items.join(Doc::from(", ") + Doc::Hardline));

print!("{}", pprint(doc, PRINTER));
// prints:
// [
//   1,
//   2,
//   3
// ]
```

## Document Model

The document model provides a rich set of building blocks:

-   Primitive values like strings, numbers
-   Containers like vectors, tuples, maps, sets
-   Formatting like `concat`, `join`, `smart_join`, `wrap`, `group`
-   Indentation control with `indent` and `dedent`
-   Conditional formatting with `if_break`
-   Line breaks like `hardline`, `softline`

The `Printer` handles pretty printing a `Doc` to a string with configurable options:

-   `max_width` - maximum width of each line
-   `indent` - number of spaces for each indentation level
-   `use_tabs` - use tabs instead of spaces for indentation

Two entry points:

-   `pprint(doc, printer)` — consumes the `Doc` and renders to `String`
-   `pprint_ref(&doc, printer)` — borrows the `Doc` without consuming or cloning it; useful for benchmarks and repeated renders (e.g., LSP formatting)

## Derive Macro

The derive macro allows for easy pretty printing of essentially any type. Here's a
trivial example:

```rust
#[derive(Pretty)]
struct Point {
    x: f64,
    y: f64
}

let point = Point { x: 1.0, y: 2.0 };
print!("{}", Doc::from(point)); // prints "(x: 1, y: 2)"
```

`Pretty` supports an additional attribute, `pprint`, which is used to customize an
object's pretty printing definition. The following options are available:

-   skip: bool: Skip this field - don't include it in the output
-   indent: bool: Indent this field - add a newline and indent before and after
-   rename: Option<String>: Rename this field - use the given string as the field name
-   getter: Option<String>: Use the given function to get the value of this field
-   verbose: bool: Verbose output - include field names in output

```rust
#[derive(Pretty)]
#[pprint(verbose)]
struct Point {
    #[pprint(rename = "x-coordinate")]
    x: f64,
    #[pprint(rename = "y-coordinate")]
    y: f64
    #[pprint(skip)]
    _skip_me: bool,
}

let point = Point { x: 1.0, y: 2.0, _skip_me: true };

print!("{}", Doc::from(point));

// prints:
// Point {
//   x-coordinate: 1,
//   y-coordinate: 2
// }
```

Structures can be arbitrarily nested. More involved examples can be found in
the [tests](tests/derive_tests.rs) file.

## `smart_join`

`smart_join`'s implementation is based off the text justification algorithm: [`text_justify`](src/utils)

`text_justify` uses greedy bin-packing: items are packed left-to-right into lines, breaking when the next item would exceed `max_width`. O(n) for all inputs.

For more information on the algorithm in particular, see the above's heavily commented source code, or the wonderful [Lecture No. 20](https://www.youtube.com/watch?v=ENyox7kNKeY) from MIT's 6.006 course, "Introduction to Algorithms".

## Performance

Throughput varies by workload -- leaf-heavy documents (integers, strings) are close to `Debug`, while `smart_join` adds DP overhead for optimal line breaking.

| Benchmark | pprint (ns) | Debug (ns) | Ratio |
|-----------|-------------|------------|-------|
| flat_vec_1k (ints) | 33,025 | 21,446 | 1.5x |
| flat_vec_10k (ints) | 307,533 | 222,306 | 1.4x |
| nested_100x100 | 293,817 | 327,671 | 0.90x |
| floats_1k | 43,227 | 67,748 | 0.64x |
| strings_1k | 109,014 | 48,582 | 2.2x |
| tuples_1k | 270,835 | 153,970 | 1.8x |

### Render Optimizations

Several optimizations target the render pipeline, particularly for code formatting
workloads (CSS, JSON) where throughput exceeds 1,400 MB/s on the Doc-to-string phase:

- **No full-tree pre-pass.** Output buffer starts at fixed 1024 capacity.
  `count_text_length` is called lazily by `Group` and `Join` only when needed.
- **`LinearJoin` variant.** Inline break decisions during render with no
  `text_justify` pre-pass. Best for code formatting where optimal prose
  justification is unnecessary. Emitted by bbnf codegen for non-text-justify modes.
- **Reverse cursor in `SmartJoin`.** Break checks use a reverse-iterating cursor
  (O(1) per item) instead of `binary_search` (O(log n)).
- **FxHashMap pre-allocated.** `text_length_cache` uses `rustc-hash` FxHashMap
  with 256 initial capacity, avoiding rehash overhead.
- **`text_justify` memo pooling.** The `doc_lengths` buffer is reused across
  `SmartJoin` calls (cleared, not reallocated).
- **Release `from_utf8_unchecked`.** All Doc sources produce valid UTF-8, so
  release builds skip validation.

In the [gorgeous](https://github.com/mkbabb/gorgeous) CSS formatter, the pprint
render phase achieves ~1,428 MB/s on bootstrap.css (281KB), making it negligible
compared to parsing and Doc construction.

See the [benches](benches) directory for more information.

## About

Contributions and suggestions welcome — open an issue or pull request.
