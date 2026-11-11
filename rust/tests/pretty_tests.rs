use pprint::{PRINTER, Printer, pprint};

#[test]
fn test_integers() {
    assert_eq!(pprint(&0i32, PRINTER), "0");
    assert_eq!(pprint(&42i64, PRINTER), "42");
    assert_eq!(pprint(&-1i8, PRINTER), "-1");
    assert_eq!(pprint(&u64::MAX, PRINTER), u64::MAX.to_string());
    assert_eq!(pprint(&i128::MIN, PRINTER), i128::MIN.to_string());
    assert_eq!(pprint(&u128::MAX, PRINTER), u128::MAX.to_string());
}

#[test]
fn test_floats() {
    let out = pprint(&2.5f64, PRINTER);
    assert!(out.starts_with("2.5"), "got: {out}");

    let out = pprint(&0.0f64, PRINTER);
    assert_eq!(out, "0.0");

    let out = pprint(&-1.5f64, PRINTER);
    assert!(out.starts_with("-1.5"), "got: {out}");
}

#[test]
fn test_bool() {
    assert_eq!(pprint(&true, PRINTER), "true");
    assert_eq!(pprint(&false, PRINTER), "false");
}

#[test]
fn test_str() {
    let s = "hello world";
    assert_eq!(pprint(&s, PRINTER), "\"hello world\"");

    let s = String::from("owned");
    assert_eq!(pprint(&s, PRINTER), "\"owned\"");
}

#[test]
fn test_vec_empty() {
    let v: Vec<i32> = vec![];
    assert_eq!(pprint(&v, PRINTER), "[]");
}

#[test]
fn test_vec_single() {
    let v = vec![42];
    assert_eq!(pprint(&v, PRINTER), "[42]");
}

#[test]
fn test_vec_fits() {
    let v = vec![1, 2, 3];
    assert_eq!(pprint(&v, PRINTER), "[1, 2, 3]");
}

#[test]
fn test_vec_breaks() {
    let v: Vec<i32> = (1..=20).collect();
    let out = pprint(&v, Printer::new(30, 4, false));
    // Should break across multiple lines at narrow width
    assert!(out.contains('\n'), "expected multiline output: {out}");
    // Should have proper indentation
    assert!(out.contains("    "), "expected indented lines: {out}");
}

#[test]
fn test_tuple() {
    let t = (1i32, 2i32);
    assert_eq!(pprint(&t, PRINTER), "(1, 2)");
}

#[test]
fn test_nested_vec() {
    let v = vec![vec![1, 2], vec![3, 4]];
    let out = pprint(&v, PRINTER);
    assert!(out.contains("[1, 2]"), "got: {out}");
    assert!(out.contains("[3, 4]"), "got: {out}");
}

#[test]
fn test_width_40_vs_120() {
    let v: Vec<i32> = (1..=30).collect();

    let wide = pprint(&v, Printer::new(120, 4, false));
    let narrow = pprint(&v, Printer::new(40, 4, false));

    let wide_lines = wide.lines().count();
    let narrow_lines = narrow.lines().count();

    assert!(
        narrow_lines > wide_lines,
        "narrow ({narrow_lines} lines) should break more than wide ({wide_lines})\nnarrow:\n{narrow}\nwide:\n{wide}"
    );
}

#[test]
fn test_option() {
    assert_eq!(pprint(&Some(42i32), PRINTER), "42");
    assert_eq!(pprint(&None::<i32>, PRINTER), "None");
}

#[test]
fn test_boxed() {
    let b = Box::new(42i32);
    assert_eq!(pprint(&b, PRINTER), "42");
}
