# `pprint`

A Rust library for pretty printing using a document model.

## Features

-   Document model for representing formatted text
-   Pretty printing to string with configurable options
-   Derive macros for automatic implementation

## Usage

```rust
use pprint::{Doc, Printer, PRINTER};

let doc = Doc::from(vec![1, 2, 3])
    .wrap("[", "]")
    .join(", ");

print!("{}", PRINTER.pretty(doc));
// prints:
// [
//   1,
//   2,
//   3
// ]
```

The document model provides a rich set of building blocks:

-   Primitive values like strings, numbers
-   Containers like vectors, tuples, maps, sets
-   Formatting like `concat`, `join`, `wrap`, `group`
-   Indentation control with `indent` and `dedent`
-   Conditional formatting with `if_break`
-   Line breaks like `hardline`, `softline`

The `Printer` handles pretty printing a `Doc` to a string with configurable options:

-   `max_width` - maximum width of each line
-   `indent` - number of spaces for each indentation level
-   `break_long_text` - insert line breaks for long text
-   `use_tabs` - use tabs instead of spaces for indentation

There are also derive macros included for easy implementation:

```rust
#[derive(Pretty)]
struct Point {
    x: f64,
    y: f64
}

let point = Point { x: 1.0, y: 2.0 };
print!("{}", point.to_doc()); // prints "(x: 1, y: 2)"
```
