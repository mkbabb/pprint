# `pprint`

A Rust library for pretty ✨ printing using a document model. Automatically derive
`Pretty` for structs, enums, and primitive types; vector and map types are also
supported by default; very similar to the `derive(Debug)` macro, just prettier and more
configurable.

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

## Document Model

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

## Derive Macro

Half of the library's development time was spent on the derive macro, allowing for easy
pretty printing of essentially any type. Here's a trivial example:

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

Structures can be arbitrarily nested, & c. & c. More involved examples can be found in
the [tests](tests/derive_tests.rs) file.

## About

This library was partway created as a means by which to learn more about Rust's procedural macros, and partway because I just love pretty printing. It's a work in progress, but I'm fairly pleased with it hitherto. If you have any suggestions, please feel free to open an issue or a pull request.

Also of note is the implementation of the `text_justify` function, which was based off the wonderful series of lectures (specifically [No. 20](https://www.youtube.com/watch?v=ENyox7kNKeY)) from MIT's 6.006 course, "Introduction to Algorithms". It's a simple and rather elegant algorithm that I've found to be quite effective in practice.
