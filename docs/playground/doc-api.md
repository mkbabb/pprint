---
title: Doc API
order: 21
section: pprint
---

# Doc API

The `Doc<'a>` enum is pprint's core type. Every document is a tree of `Doc` nodes that the renderer traverses to produce output.

## Text Variants

pprint uses specialized variants to avoid heap allocation for short strings:

```rust
Doc::Char(b'x')              // 1 byte — stored inline
Doc::DoubleChar([b'i', b'f'])  // 2 bytes
Doc::TripleChar(...)          // 3 bytes
Doc::QuadChar(...)            // 4 bytes
Doc::SmallBytes([u8; 24], len) // up to 24 bytes on the stack
Doc::Bytes(Vec<u8>, len)      // heap-allocated for longer text
Doc::String(Cow<'a, str>)     // borrowed or owned string
```

Numeric types are stored natively and formatted at render time using fast integer/float formatters (`itoap`, `dragonbox`):

```rust
Doc::i32(42)    // rendered as "42"
Doc::f64(3.14)  // rendered as "3.14"
```

The `From` trait converts common types automatically:

```rust
let d: Doc = "hello".into();        // &str → SmallBytes
let d: Doc = String::from("world").into(); // String → Bytes
let d: Doc = 42i32.into();          // i32 → Doc::i32
let d: Doc = true.into();           // bool → SmallBytes("true")
```

## Structural Variants

### Concat

Sequence of documents, printed left to right. The `+` operator builds `Concat` nodes:

```rust
let doc = Doc::from("hello") + Doc::from(" ") + Doc::from("world");
// Equivalent to: Doc::Concat(vec![...])
```

For 2 or 3 elements, pprint uses `DoubleDoc` / `TripleDoc` to avoid the `Vec` allocation:

```rust
Doc::DoubleDoc(Box::new(a), Box::new(b))
Doc::TripleDoc(Box::new(a), Box::new(b), Box::new(c))
```

### Group

Try to print contents flat; enter break mode if they exceed `max_width`:

```rust
use pprint::group;

let doc = group(Doc::from("[") + inner + Doc::from("]"));
```

### Indent / Dedent

Increase or decrease the indentation level for nested content:

```rust
use pprint::{indent, dedent};

let doc = indent(Doc::from("nested content"));
```

### Hardline, Softline, Mediumline, Line

```rust
Doc::Hardline   // Always breaks — unconditional newline + indent
Doc::Softline   // Breaks only when current_line_len > max_width
Doc::Mediumline // Breaks when current_line_len > max_width / 2
Doc::Line       // Raw newline (no indent)
```

### IfBreak

Conditional output based on whether the enclosing `Group` broke:

```rust
use pprint::if_break;

// When Group breaks: emit ",\n"; when flat: emit ", "
let sep = if_break(
    Doc::from(",") + Doc::Hardline,
    Doc::from(", "),
);
```

## Join Variants

All three Join variants use a **tuple-boxed** form `Box<(Doc, Vec<Doc>)>` to keep the enum size small (a single pointer instead of inlining a separator + Vec).

### Join

Simple interleaving — inserts the separator between every pair of documents:

```rust
use pprint::join;

let doc = join(Doc::from(", "), vec![Doc::from("a"), Doc::from("b"), Doc::from("c")]);
// Renders: "a, b, c"
```

### SmartJoin

Uses a DP text-justification algorithm to decide where to insert line breaks, minimizing total overflow. Ideal for lists where items vary in width:

```rust
use pprint::smart_join;

let doc = smart_join(Doc::from(", "), items);
```

### LinearJoin

Makes break decisions with a single forward scan. No text-justification pre-pass — each item is simply placed on the current line if it fits, or wrapped to the next line if not. Faster than SmartJoin for cases where optimal packing is not critical:

```rust
use pprint::linear_join;

let doc = linear_join(Doc::from(", "), items);
```

## Trait Methods

`Doc` implements several traits for fluent construction:

```rust
use pprint::{Group, Indent, Dedent, Join, SmartJoin, LinearJoin, Wrap};

let doc = items.join(", ");                   // Vec<Doc>.join(sep)
let doc = items.smart_join(", ");             // Vec<Doc>.smart_join(sep)
let doc = items.linear_join(", ");            // Vec<Doc>.linear_join(sep)
let doc = inner.group();                      // Doc.group()
let doc = inner.indent();                     // Doc.indent()
let doc = inner.wrap("[", "]");               // Doc.wrap(left, right)
```

## Building a Doc Tree

Here is a complete example that formats a key-value record:

```rust
use pprint::*;

fn format_record(fields: Vec<(&str, &str)>) -> Doc<'_> {
    let entries: Vec<Doc> = fields
        .into_iter()
        .map(|(k, v)| Doc::from(k) + Doc::from(": ") + Doc::from(v))
        .collect();

    group(
        Doc::from("{")
            + indent(
                Doc::Hardline
                    + entries.join(Doc::from(",") + Doc::Hardline)
            )
            + Doc::Hardline
            + Doc::from("}")
    )
}

let doc = format_record(vec![("name", "\"Alice\""), ("age", "30")]);
let output = pprint(doc, Printer::new(40, 2, false));
// {
//   name: "Alice",
//   age: 30
// }
```

When the record fits on one line (wide enough `max_width`), the Group renders it flat: `{ name: "Alice", age: 30 }`.
