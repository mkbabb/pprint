use pprint::{PRINTER, Pretty, Printer, pprint};

// ── Test types ──────────────────────────────────────────────────────────────

#[derive(Pretty, Debug)]
struct SimpleStruct {
    x: i32,
    y: i32,
}

#[derive(Pretty, Debug)]
struct TupleStruct(i32, f64);

#[derive(Pretty, Debug)]
struct UnitStruct;

#[derive(Pretty, Debug)]
#[pprint(verbose)]
struct VerboseStruct {
    name: String,
    count: u64,
}

#[derive(Pretty, Debug)]
#[pprint(verbose, rename = "MyStruct")]
struct RenamedStruct {
    value: i32,
}

#[derive(Pretty, Debug)]
struct SkipStruct {
    a: i32,
    #[pprint(skip)]
    _b: i32,
    c: i32,
}

#[derive(Pretty, Debug)]
#[pprint(verbose)]
enum TestEnum {
    UnitA,
    #[pprint(rename = "CustomB")]
    UnitB,
    Single(i32),
    Multi(i32, i32),
    #[pprint(skip)]
    Skipped(i32),
}

#[derive(Pretty, Debug)]
struct Nested {
    inner: SimpleStruct,
    value: i32,
}

#[derive(Pretty, Debug)]
struct GenericStruct<T> {
    value: T,
}

// ── Struct tests ────────────────────────────────────────────────────────────

#[test]
fn test_struct_simple() {
    let s = SimpleStruct { x: 1, y: 2 };
    let out = pprint(&s, PRINTER);
    assert_eq!(out, "{x: 1, y: 2}");
}

#[test]
fn test_struct_unnamed() {
    let s = TupleStruct(42, 3.14);
    let out = pprint(&s, PRINTER);
    assert!(out.contains("field_0: 42"), "got: {out}");
    assert!(out.contains("field_1: 3.14"), "got: {out}");
}

#[test]
fn test_struct_unit() {
    let s = UnitStruct;
    let out = pprint(&s, PRINTER);
    assert_eq!(out, "UnitStruct");
}

#[test]
fn test_struct_verbose() {
    let s = VerboseStruct {
        name: "hello".into(),
        count: 42,
    };
    let out = pprint(&s, PRINTER);
    assert!(out.starts_with("VerboseStruct"), "got: {out}");
    assert!(out.contains("name: hello"), "got: {out}");
    assert!(out.contains("count: 42"), "got: {out}");
}

#[test]
fn test_struct_rename() {
    let s = RenamedStruct { value: 7 };
    let out = pprint(&s, PRINTER);
    assert!(out.starts_with("MyStruct"), "got: {out}");
    assert!(out.contains("value: 7"), "got: {out}");
}

#[test]
fn test_struct_skip() {
    let s = SkipStruct {
        a: 1,
        _b: 99,
        c: 3,
    };
    let out = pprint(&s, PRINTER);
    assert!(out.contains("a: 1"), "got: {out}");
    assert!(!out.contains("99"), "skipped field should not appear: {out}");
    assert!(out.contains("c: 3"), "got: {out}");
}

// ── Enum tests ──────────────────────────────────────────────────────────────

#[test]
fn test_enum_unit() {
    let e = TestEnum::UnitA;
    let out = pprint(&e, PRINTER);
    assert_eq!(out, "UnitA");
}

#[test]
fn test_enum_unit_renamed() {
    let e = TestEnum::UnitB;
    let out = pprint(&e, PRINTER);
    assert_eq!(out, "CustomB");
}

#[test]
fn test_enum_single() {
    let e = TestEnum::Single(42);
    let out = pprint(&e, PRINTER);
    assert!(out.contains("Single"), "got: {out}");
    assert!(out.contains("42"), "got: {out}");
}

#[test]
fn test_enum_multi() {
    let e = TestEnum::Multi(1, 2);
    let out = pprint(&e, PRINTER);
    assert!(out.contains("Multi"), "got: {out}");
    assert!(out.contains("1"), "got: {out}");
    assert!(out.contains("2"), "got: {out}");
}

#[test]
#[should_panic(expected = "pprint(skip)")]
fn test_enum_skip_panics() {
    let e = TestEnum::Skipped(42);
    pprint(&e, PRINTER);
}

// ── Nesting and generics ────────────────────────────────────────────────────

#[test]
fn test_nested_derive() {
    let s = Nested {
        inner: SimpleStruct { x: 10, y: 20 },
        value: 42,
    };
    let out = pprint(&s, PRINTER);
    assert!(out.contains("inner:"), "got: {out}");
    assert!(out.contains("x: 10"), "got: {out}");
    assert!(out.contains("value: 42"), "got: {out}");
}

#[test]
fn test_generic_struct() {
    let s = GenericStruct { value: 42i32 };
    let out = pprint(&s, PRINTER);
    assert!(out.contains("value: 42"), "got: {out}");
}

// ── Width sensitivity ───────────────────────────────────────────────────────

#[test]
fn test_width_sensitivity() {
    let s = Nested {
        inner: SimpleStruct { x: 100, y: 200 },
        value: 300,
    };

    let wide = pprint(&s, Printer::new(120, 4, false));
    let wide_lines = wide.lines().count();

    let narrow = pprint(&s, Printer::new(20, 4, false));
    let narrow_lines = narrow.lines().count();

    assert!(
        narrow_lines > wide_lines,
        "narrow ({narrow_lines} lines) should have more lines than wide ({wide_lines})\nnarrow:\n{narrow}\nwide:\n{wide}"
    );
}
