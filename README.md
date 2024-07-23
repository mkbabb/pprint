# `pprint`

A Rust library for pretty ✨ printing using a document model. Automatically derive
`Pretty` for structs, enums, and primitive types; vector and map types are also
supported by default; very similar to the `derive(Debug)` macro, just prettier ✨ and more
configurable.

## Usage

```rust
use pprint::{Doc, pprint};

let doc = Doc::from(vec![1, 2, 3])
    .join(Doc::from(", ") + Doc::Hardline)
    .wrap("[", "]");

print!("{}", pprint(doc));
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

## `smart_join`

`smart_join`'s implementation is based off the text justification algorithm: [`text_justify`](src/utils)

For more information on the algorithm in particular, see the above's heavily commented source code, or the wonderful [Lecture No. 20](https://www.youtube.com/watch?v=ENyox7kNKeY) from MIT's 6.006 course, "Introduction to Algorithms".

## Performance

Several optimizations have been made to avoid unnecessary allocations and copying, but the library is still not as fast as `std::fmt::Debug`. This is mainly due to the document model's generality, but more optimizations are planned for the future (if I can figure it out 😅).

On average, `pprint` is anywhere from `0.25-3x` slower than `Debug`, especially if using `smart_join` (which, I think just looks nicer normally):

#### Formatting a 100 nested vectors of 100 floats each

##### `smart_join`

```shell
test bench_pprint_medium_vector ... bench:   1,303,418 ns/iter (+/- 24,732)
```

##### `join`

```shell
test bench_pprint_medium_vector ... bench:     668,400 ns/iter (+/- 13,639)
```

##### `std::fmt::Debug`

```shell
test bench_debug_medium_vector  ... bench:     459,229 ns/iter (+/- 24,587)
```

See the [benches](benches) directory for more information.

## About

This library was partway created as a means by which to learn more about Rust's procedural macros, and partway because I just love pretty printing. It's a work in progress, but I'm fairly pleased with it hitherto. If you have any suggestions, please feel free to open an issue or a pull request.
