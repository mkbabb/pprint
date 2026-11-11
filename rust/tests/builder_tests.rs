use pprint::{Doc, FmtBuilder, PRINTER, Printer, render};

fn r(doc: &Doc<'_>, max_width: usize) -> String {
    render(
        doc,
        Printer {
            max_width,
            ..PRINTER
        },
    )
}

// ── Basic ops ───────────────────────────────────────────────────────────────

#[test]
fn test_basic_text() {
    let mut b = FmtBuilder::new();
    b.text("hello world");
    assert_eq!(r(&b.finish().unwrap(), 80), "hello world");
}

#[test]
fn test_group_fits() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("a");
        b.softline();
        b.text("b");
    });
    assert_eq!(r(&b.finish().unwrap(), 80), "a b");
}

#[test]
fn test_group_breaks() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("aaaa");
        b.softline();
        b.text("bbbb");
    });
    assert_eq!(r(&b.finish().unwrap(), 6), "aaaa\nbbbb");
}

#[test]
fn test_indent() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.char(b'{');
        b.indent(|b| {
            b.softline();
            b.text("x");
        });
        b.softline();
        b.char(b'}');
    });
    assert_eq!(r(&b.finish().unwrap(), 3), "{\n    x\n}");
}

#[test]
fn test_hardline_forces_break() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("a");
        b.hardline();
        b.text("b");
    });
    assert_eq!(r(&b.finish().unwrap(), 80), "a\nb");
}

#[test]
fn test_nested_groups() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("[");
        b.group(|b| {
            b.text("a");
            b.softline();
            b.text("b");
        });
        b.text("]");
    });
    assert_eq!(r(&b.finish().unwrap(), 80), "[a b]");
}

#[test]
fn test_if_break() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("aaa");
        b.if_break(
            |b| {
                b.hardline();
                b.text("bbb");
            },
            |b| {
                b.char(b' ');
                b.text("bbb");
            },
        );
    });
    let ops = b.finish().unwrap();
    assert_eq!(r(&ops, 80), "aaa bbb");
    assert_eq!(r(&ops, 5), "aaa\nbbb");
}

// ── Numeric ops ─────────────────────────────────────────────────────────────

#[test]
fn test_numeric_i64() {
    let mut b = FmtBuilder::new();
    b.i64(42);
    assert_eq!(r(&b.finish().unwrap(), 80), "42");

    let mut b = FmtBuilder::new();
    b.i64(-1);
    assert_eq!(r(&b.finish().unwrap(), 80), "-1");

    let mut b = FmtBuilder::new();
    b.i64(i64::MIN);
    assert_eq!(r(&b.finish().unwrap(), 80), i64::MIN.to_string());
}

#[test]
fn test_numeric_u64() {
    let mut b = FmtBuilder::new();
    b.u64(0);
    assert_eq!(r(&b.finish().unwrap(), 80), "0");

    let mut b = FmtBuilder::new();
    b.u64(u64::MAX);
    assert_eq!(r(&b.finish().unwrap(), 80), u64::MAX.to_string());
}

#[test]
fn test_numeric_i128() {
    let mut b = FmtBuilder::new();
    b.i128(i128::MIN);
    assert_eq!(r(&b.finish().unwrap(), 80), i128::MIN.to_string());

    let mut b = FmtBuilder::new();
    b.i128(i128::MAX);
    assert_eq!(r(&b.finish().unwrap(), 80), i128::MAX.to_string());
}

#[test]
fn test_numeric_u128() {
    let mut b = FmtBuilder::new();
    b.u128(u128::MAX);
    assert_eq!(r(&b.finish().unwrap(), 80), u128::MAX.to_string());
}

#[test]
fn test_numeric_f64() {
    let mut b = FmtBuilder::new();
    b.f64(2.5);
    let out = r(&b.finish().unwrap(), 80);
    assert!(out.starts_with("2.5"), "got: {out}");
}

#[test]
fn test_numeric_in_group() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.i64(1);
        b.softline();
        b.i64(2);
    });
    assert_eq!(r(&b.finish().unwrap(), 80), "1 2");

    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.i64(12345);
        b.softline();
        b.i64(67890);
    });
    assert_eq!(r(&b.finish().unwrap(), 8), "12345\n67890");
}

#[test]
fn test_mixed_text_numeric() {
    let mut b = FmtBuilder::new();
    b.text("x = ");
    b.i64(42);
    b.text(", y = ");
    b.f64(2.5);
    let out = r(&b.finish().unwrap(), 80);
    assert!(out.starts_with("x = 42, y = 2.5"), "got: {out}");
}

// ── Wrap + Sep ──────────────────────────────────────────────────────────────

#[test]
fn test_wrap_fits() {
    let mut b = FmtBuilder::new();
    b.wrap(b'[', b']', |b| {
        b.i64(1);
        b.comma_sep();
        b.i64(2);
    });
    assert_eq!(r(&b.finish().unwrap(), 80), "[1, 2]");
}

#[test]
fn test_wrap_breaks() {
    let mut b = FmtBuilder::new();
    b.wrap(b'[', b']', |b| {
        b.text("aaaa");
        b.comma_sep();
        b.text("bbbb");
    });
    assert_eq!(r(&b.finish().unwrap(), 8), "[\n    aaaa,\n    bbbb\n]");
}

#[test]
fn test_comma_sep_flat() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("a");
        b.comma_sep();
        b.text("b");
        b.comma_sep();
        b.text("c");
    });
    assert_eq!(r(&b.finish().unwrap(), 80), "a, b, c");
}

#[test]
fn test_custom_sep() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text("a");
        b.sep(" | ", " |");
        b.text("b");
    });
    let ops = b.finish().unwrap();
    assert_eq!(r(&ops, 80), "a | b");
    assert_eq!(r(&ops, 3), "a |\nb");
}

// ── Join ops ────────────────────────────────────────────────────────────────

#[test]
fn test_join_empty() {
    let mut b = FmtBuilder::new();
    let items: Vec<&str> = vec![];
    b.join(items, |b, s| b.text(s));
    assert_eq!(r(&b.finish().unwrap(), 80), "");
}

#[test]
fn test_join_single() {
    let mut b = FmtBuilder::new();
    b.join(["hello"], |b, s| b.text(s));
    assert_eq!(r(&b.finish().unwrap(), 80), "hello");
}

#[test]
fn test_smart_join_basic() {
    let items = vec!["aaa", "bbb", "ccc", "ddd"];
    let mut b = FmtBuilder::new();
    b.smart_join(items, ", ", 15, 0, |b, s| b.text(s));
    let out = r(&b.finish().unwrap(), 80);
    assert!(out.contains('\n'), "expected break in: {out}");
}

#[test]
fn test_smart_join_all_fit() {
    let items = vec!["a", "b", "c"];
    let mut b = FmtBuilder::new();
    b.smart_join(items, ", ", 80, 0, |b, s| b.text(s));
    assert_eq!(r(&b.finish().unwrap(), 80), "a, b, c");
}

#[test]
fn test_linear_join_basic() {
    let items = vec!["aaa", "bbb", "ccc", "ddd"];
    let mut b = FmtBuilder::new();
    b.linear_join(items, ", ", |b, s| b.text(s));
    // The width is the renderer's now (a fill, law L5), not a build argument.
    let doc = b.finish().unwrap();
    assert_eq!(r(&doc, 12), "aaa, bbb,\nccc, ddd");
    assert_eq!(r(&doc, 80), "aaa, bbb, ccc, ddd");
}

// ── Deep nesting ────────────────────────────────────────────────────────────

#[test]
fn test_deep_nesting() {
    let mut b = FmtBuilder::new();
    for _ in 0..10 {
        b.group_open();
        b.indent_open();
        b.softline();
    }
    b.text("deep");
    for _ in 0..10 {
        b.indent_close();
        b.group_close();
    }
    let out = r(&b.finish().unwrap(), 3);
    assert!(out.contains("deep"));
    assert!(out.contains('\n'));
}

// ── use_tabs ────────────────────────────────────────────────────────────────

#[test]
fn test_use_tabs() {
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.char(b'{');
        b.indent(|b| {
            b.softline();
            b.text("x");
        });
        b.softline();
        b.char(b'}');
    });
    let ops = b.finish().unwrap();
    let out = render(
        &ops,
        Printer {
            max_width: 3,
            indent: 1,
            use_tabs: true,
        },
    );
    assert_eq!(out, "{\n\tx\n}");
}

// ── light_checkpoint / light_restore ───────────────────────────────────────

#[test]
fn test_light_checkpoint_restore() {
    let mut b = FmtBuilder::new();
    b.text("keep");
    let cp = b.light_checkpoint();
    b.text(" discard");
    b.light_restore(cp);
    b.text(" also_keep");
    assert_eq!(r(&b.finish().unwrap(), 80), "keep also_keep");
}

// ── text_inline_ws ─────────────────────────────────────────────────────────

#[test]
fn test_text_inline_ws_spaces() {
    let mut b = FmtBuilder::new();
    b.text("a");
    b.text_inline_ws("  ");
    b.text("b");
    assert_eq!(r(&b.finish().unwrap(), 80), "a  b");
}

#[test]
fn test_text_inline_ws_newline_suppressed() {
    let mut b = FmtBuilder::new();
    b.text("a");
    b.text_inline_ws(" \n ");
    b.text("b");
    // Newline-containing whitespace is suppressed entirely.
    assert_eq!(r(&b.finish().unwrap(), 80), "ab");
}

#[test]
fn test_text_inline_ws_empty() {
    let mut b = FmtBuilder::new();
    b.text("a");
    b.text_inline_ws("");
    b.text("b");
    assert_eq!(r(&b.finish().unwrap(), 80), "ab");
}

// ── flat_width above u16::MAX ──────────────────────────────────────────────

#[test]
fn test_wide_flat_width_forces_break() {
    // A group whose flat content exceeds u16::MAX (65535): ABI v2 records
    // the width exactly (u32), so at 80 columns it must break.
    let long = "x".repeat(70_000);
    let mut b = FmtBuilder::new();
    b.group(|b| {
        b.text(&long);
        b.softline();
        b.text("y");
    });
    let ops = b.finish().unwrap();
    // The group is far wider than any reasonable max_width, so it must break.
    let out = render(
        &ops,
        Printer {
            max_width: 80,
            ..PRINTER
        },
    );
    assert!(
        out.contains('\n'),
        "expected break mode, got flat: len={}",
        out.len()
    );
}
