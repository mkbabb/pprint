use rustc_hash::FxHashMap;

use crate::DigitCount;
use crate::doc::Doc;
use crate::utils::text_justify;

struct PrintItem<'a> {
    doc: &'a Doc<'a>,
    indent_delta: usize,

    left: Option<&'a Doc<'a>>,
    break_left: usize,

    /// Whether the enclosing Group decided to break.
    /// Set by Group when it decides `needs_breaking`, read by IfBreak.
    break_mode: bool,
}

impl<'a> PrintItem<'a> {
    #[inline(always)]
    fn new(doc: &'a Doc<'a>, indent_delta: usize) -> Self {
        Self {
            doc,
            indent_delta,
            left: None,
            break_left: 0,
            break_mode: false,
        }
    }
}

struct PrintState<'a> {
    stack: Vec<PrintItem<'a>>,
    output: Vec<u8>,

    current_line_len: usize,
    indent_delta: usize,

    space_cache: Vec<u8>,
    join_breaks: Vec<usize>,
    doc_lengths: Vec<usize>,
    text_length_cache: FxHashMap<*const Doc<'a>, usize>,
}

#[inline(always)]
fn is_literal_doc(doc: &Doc) -> bool {
    match doc {
        Doc::Null => false,
        Doc::Char(_)
        | Doc::DoubleChar(_)
        | Doc::TripleChar(_)
        | Doc::QuadChar(_)
        | Doc::SmallBytes(_, _)
        | Doc::Bytes(_, _)
        | Doc::String(_)
        | Doc::i8(_)
        | Doc::i16(_)
        | Doc::i32(_)
        | Doc::i64(_)
        | Doc::i128(_)
        | Doc::isize(_)
        | Doc::u8(_)
        | Doc::u16(_)
        | Doc::u32(_)
        | Doc::u64(_)
        | Doc::u128(_)
        | Doc::usize(_)
        | Doc::f32(_)
        | Doc::f64(_)
        | Doc::Line
        | Doc::Softline
        | Doc::Mediumline
        | Doc::Hardline => true,
        Doc::DoubleDoc(doc1, doc2) => is_literal_doc(doc1) && is_literal_doc(doc2),
        Doc::TripleDoc(doc1, doc2, doc3) => {
            is_literal_doc(doc1) && is_literal_doc(doc2) && is_literal_doc(doc3)
        }
        Doc::Concat(_) => false,
        Doc::Group(_) => false,
        Doc::Indent(_) => false,
        Doc::Dedent(_) => false,
        Doc::Join(_) => false,
        Doc::SmartJoin(_) => false,
        Doc::IfBreak(_, _) => false,
    }
}

#[inline(always)]
fn count_join_length<'a>(
    sep: &'a Doc<'a>,
    docs: &'a [Doc<'a>],
    printer: &Printer,
    cache: &mut FxHashMap<*const Doc<'a>, usize>,
) -> usize {
    if docs.is_empty() {
        return 0;
    }

    let doc_len: usize = docs
        .iter()
        .map(|d| count_text_length(d, printer, cache))
        .sum();
    let sep_len = count_text_length(sep, printer, cache);

    doc_len + sep_len * (docs.len() - 1)
}

#[inline(always)]
fn literal_text_length(doc: &Doc, printer: &Printer) -> Option<usize> {
    match doc {
        Doc::Null => Some(0),
        Doc::Char(_) => Some(1),
        Doc::DoubleChar(_) => Some(2),
        Doc::TripleChar(_) => Some(3),
        Doc::QuadChar(_) => Some(4),
        Doc::SmallBytes(_, len) => Some(*len),
        Doc::Bytes(_, len) => Some(*len),
        Doc::String(s) => Some(s.len()),
        Doc::i8(value) => Some(value.len()),
        Doc::i16(value) => Some(value.len()),
        Doc::i32(value) => Some(value.len()),
        Doc::i64(value) => Some(value.len()),
        Doc::i128(value) => Some(value.len()),
        Doc::isize(value) => Some(value.len()),
        Doc::u8(value) => Some(value.len()),
        Doc::u16(value) => Some(value.len()),
        Doc::u32(value) => Some(value.len()),
        Doc::u64(value) => Some(value.len()),
        Doc::u128(value) => Some(value.len()),
        Doc::usize(value) => Some(value.len()),
        Doc::f32(value) => {
            assert!(
                value.is_finite(),
                "pprint: non-finite float is unsupported (value: {value})"
            );
            Some(10)
        }
        Doc::f64(value) => {
            assert!(
                value.is_finite(),
                "pprint: non-finite float is unsupported (value: {value})"
            );
            Some(20)
        }
        Doc::Softline => Some(1),
        Doc::Mediumline => Some(0),
        Doc::Hardline | Doc::Line => Some(printer.max_width),
        Doc::DoubleDoc(doc1, doc2) => {
            Some(literal_text_length(doc1, printer)? + literal_text_length(doc2, printer)?)
        }
        Doc::TripleDoc(doc1, doc2, doc3) => Some(
            literal_text_length(doc1, printer)?
                + literal_text_length(doc2, printer)?
                + literal_text_length(doc3, printer)?,
        ),
        Doc::Concat(_)
        | Doc::Group(_)
        | Doc::Indent(_)
        | Doc::Dedent(_)
        | Doc::Join(_)
        | Doc::SmartJoin(_)
        | Doc::IfBreak(_, _) => None,
    }
}

#[inline]
fn count_text_length<'a>(
    doc: &'a Doc<'a>,
    printer: &Printer,
    cache: &mut FxHashMap<*const Doc<'a>, usize>,
) -> usize {
    if let Some(len) = literal_text_length(doc, printer) {
        return len;
    }

    let key = doc as *const _;
    if let Some(&len) = cache.get(&key) {
        return len;
    }
    let len = match doc {
        Doc::Concat(docs) => docs
            .iter()
            .map(|d| count_text_length(d, printer, cache))
            .sum(),

        Doc::DoubleDoc(doc1, doc2) => {
            count_text_length(doc1, printer, cache) + count_text_length(doc2, printer, cache)
        }
        Doc::TripleDoc(doc1, doc2, doc3) => {
            count_text_length(doc1, printer, cache)
                + count_text_length(doc2, printer, cache)
                + count_text_length(doc3, printer, cache)
        }

        Doc::Group(d) => count_text_length(d, printer, cache),

        Doc::Indent(d) => count_text_length(d, printer, cache).saturating_add(printer.indent),
        Doc::Dedent(d) => count_text_length(d, printer, cache).saturating_sub(printer.indent),

        Doc::Join(inner) => count_join_length(&inner.0, &inner.1, printer, cache),

        Doc::SmartJoin(inner) => count_join_length(&inner.0, &inner.1, printer, cache),

        // Use the "fits" branch for width calculation — we're measuring
        // whether the enclosing Group fits inline (break_mode=false).
        Doc::IfBreak(_t, f) => count_text_length(f, printer, cache),

        Doc::Null
        | Doc::Char(_)
        | Doc::DoubleChar(_)
        | Doc::TripleChar(_)
        | Doc::QuadChar(_)
        | Doc::SmallBytes(_, _)
        | Doc::Bytes(_, _)
        | Doc::String(_)
        | Doc::i8(_)
        | Doc::i16(_)
        | Doc::i32(_)
        | Doc::i64(_)
        | Doc::i128(_)
        | Doc::isize(_)
        | Doc::u8(_)
        | Doc::u16(_)
        | Doc::u32(_)
        | Doc::u64(_)
        | Doc::u128(_)
        | Doc::usize(_)
        | Doc::f32(_)
        | Doc::f64(_)
        | Doc::Softline
        | Doc::Mediumline
        | Doc::Hardline
        | Doc::Line => unreachable!("literal docs are handled in literal_text_length"),
    };
    cache.insert(key, len);
    len
}

#[inline(always)]
fn smart_join_breaks<'a>(
    sep: &'a Doc<'a>,
    docs: &'a [Doc<'a>],

    state: &mut PrintState<'a>,
    printer: &mut Printer,
) {
    let max_width = printer.max_width.saturating_sub(state.indent_delta);

    let sep_length = count_text_length(sep, printer, &mut state.text_length_cache);
    state.doc_lengths.clear();
    state.doc_lengths.extend(
        docs.iter()
            .map(|d| count_text_length(d, printer, &mut state.text_length_cache)),
    );

    // sep_length stays as-is — text_justify already accounts for separators.

    state.join_breaks.clear();

    text_justify(
        sep_length,
        &state.doc_lengths,
        max_width,
        &mut state.join_breaks,
    )
}

#[inline(always)]
fn format_int<T>(value: T, state: &mut PrintState) -> usize
where
    T: itoap::Integer + std::fmt::Display,
{
    let prev_len = state.output.len();
    itoap::write_to_vec(&mut state.output, value);
    state.output.len() - prev_len
}

#[inline(always)]
fn format_f64(value: f64, state: &mut PrintState) -> usize {
    assert!(
        value.is_finite(),
        "pprint: non-finite float is unsupported (value: {value})"
    );
    let mut buf = dragonbox::Buffer::new();
    let s = buf.format_finite(value).as_bytes();
    state.output.extend_from_slice(s);
    s.len()
}

#[inline(always)]
fn format_f32(value: f32, state: &mut PrintState) -> usize {
    format_f64(value as f64, state)
}

#[inline(always)]
fn append_line(state: &mut PrintState, printer: &mut Printer) -> usize {
    let space_cache = &mut state.space_cache;

    let indent_delta = state.indent_delta;

    if space_cache.is_empty() {
        space_cache.push(b'\n');
    }

    if indent_delta >= space_cache.len() {
        let space = if printer.use_tabs { b'\t' } else { b' ' };
        for _ in space_cache.len()..=indent_delta {
            space_cache.push(space);
        }
    }

    // space_cache layout: ['\n', ' ', ' ', ' ', ...] — index 0 is the newline,
    // indices 1..=indent_delta are indent spaces.  Output indent_delta + 1 bytes
    // to get newline + indent_delta spaces.
    let output_len = indent_delta + 1;
    state.output.extend_from_slice(&space_cache[..output_len]);

    indent_delta
}

#[inline(always)]
fn handle_line<'a>(doc: &'a Doc<'a>, state: &mut PrintState<'a>, printer: &mut Printer) -> usize {
    match doc {
        Doc::Line => {
            state.output.push(b'\n');
            0
        }

        Doc::Hardline => append_line(state, printer),

        Doc::Mediumline if state.current_line_len > printer.max_width / 2 => {
            append_line(state, printer)
        }
        Doc::Mediumline => state.current_line_len,

        Doc::Softline if state.current_line_len > printer.max_width => append_line(state, printer),
        Doc::Softline => state.current_line_len,

        _ => panic!("handle_line called with non-line Doc variant"),
    }
}

#[inline(always)]
fn handle_literal<'a>(doc: &'a Doc<'a>, state: &mut PrintState<'a>, printer: &mut Printer) {
    let offset = match doc {
        Doc::Null => 0,

        Doc::Char(c) => {
            state.output.push(*c);
            1
        }
        Doc::DoubleChar(cs) => {
            state.output.extend_from_slice(cs);
            2
        }
        Doc::TripleChar(cs) => {
            state.output.extend_from_slice(cs);
            3
        }
        Doc::QuadChar(cs) => {
            state.output.extend_from_slice(cs);
            4
        }

        Doc::SmallBytes(b, len) => {
            state.output.extend_from_slice(&b[..*len]);
            *len
        }

        Doc::Bytes(b, len) => {
            state.output.extend_from_slice(b);
            *len
        }

        Doc::String(s) => {
            state.output.extend_from_slice(s.as_bytes());
            s.len()
        }

        Doc::i8(v) => format_int(*v, state),
        Doc::i16(v) => format_int(*v, state),
        Doc::i32(v) => format_int(*v, state),
        Doc::i64(v) => format_int(*v, state),
        Doc::i128(v) => format_int(*v, state),
        Doc::isize(v) => format_int(*v, state),

        Doc::u8(v) => format_int(*v, state),
        Doc::u16(v) => format_int(*v, state),
        Doc::u32(v) => format_int(*v, state),
        Doc::u64(v) => format_int(*v, state),
        Doc::u128(v) => format_int(*v, state),
        Doc::usize(v) => format_int(*v, state),

        Doc::f32(v) => format_f32(*v, state),
        Doc::f64(v) => format_f64(*v, state),

        Doc::Line | Doc::Softline | Doc::Mediumline | Doc::Hardline => 0,

        Doc::DoubleDoc(_, _) | Doc::TripleDoc(_, _, _) => 0,

        Doc::Concat(_)
        | Doc::Group(_)
        | Doc::Indent(_)
        | Doc::Dedent(_)
        | Doc::Join(_)
        | Doc::SmartJoin(_)
        | Doc::IfBreak(_, _) => {
            panic!("handle_literal called with non-literal Doc variant")
        }
    };

    state.current_line_len = match doc {
        Doc::Line | Doc::Hardline | Doc::Mediumline | Doc::Softline => {
            handle_line(doc, state, printer)
        }
        _ => state.current_line_len + offset,
    };

    match doc {
        Doc::DoubleDoc(doc1, doc2) => {
            handle_literal(doc1, state, printer);
            handle_literal(doc2, state, printer);
        }
        Doc::TripleDoc(doc1, doc2, doc3) => {
            handle_literal(doc1, state, printer);
            handle_literal(doc2, state, printer);
            handle_literal(doc3, state, printer);
        }
        Doc::Null
        | Doc::Char(_)
        | Doc::DoubleChar(_)
        | Doc::TripleChar(_)
        | Doc::QuadChar(_)
        | Doc::SmallBytes(_, _)
        | Doc::Bytes(_, _)
        | Doc::String(_)
        | Doc::i8(_)
        | Doc::i16(_)
        | Doc::i32(_)
        | Doc::i64(_)
        | Doc::i128(_)
        | Doc::isize(_)
        | Doc::u8(_)
        | Doc::u16(_)
        | Doc::u32(_)
        | Doc::u64(_)
        | Doc::u128(_)
        | Doc::usize(_)
        | Doc::f32(_)
        | Doc::f64(_)
        | Doc::Line
        | Doc::Softline
        | Doc::Mediumline
        | Doc::Hardline => {}
        Doc::Concat(_)
        | Doc::Group(_)
        | Doc::Indent(_)
        | Doc::Dedent(_)
        | Doc::Join(_)
        | Doc::SmartJoin(_)
        | Doc::IfBreak(_, _) => {
            panic!("handle_literal reached non-literal composite variant")
        }
    }
}

fn handle_join<'a>(
    doc: &'a Doc<'a>,
    sep: &'a Doc<'a>,
    docs: &'a [Doc<'a>],
    state: &mut PrintState<'a>,
    printer: &mut Printer,
) {
    let is_smart_join = matches!(doc, Doc::SmartJoin(_));

    if is_smart_join {
        smart_join_breaks(sep, docs, state, printer);
    } else {
        state.join_breaks.clear();
    }

    let sep_is_lit = is_literal_doc(sep);

    for (i, d) in docs.iter().rev().enumerate() {
        let i = docs.len() - i - 1;

        let left = if i > 0 && sep_is_lit { Some(sep) } else { None };

        let break_left = if is_smart_join && state.join_breaks.binary_search(&i).is_ok() {
            state.indent_delta
        } else {
            0
        };

        state.stack.push(PrintItem {
            doc: d,
            indent_delta: state.indent_delta,
            left,
            break_left,
            break_mode: false,
        });

        if !sep_is_lit && i > 0 {
            state.stack.push(PrintItem {
                doc: sep,
                indent_delta: state.indent_delta,
                left: None,
                break_left,
                break_mode: false,
            });
        }
    }
}

fn handle_n_docs_unrolled<'a>(doc: &'a Doc<'a>, state: &mut PrintState<'a>, printer: &mut Printer) {
    match doc {
        Doc::DoubleDoc(doc1, doc2) => {
            let doc1_is_lit = is_literal_doc(doc1);
            let doc2_is_lit = is_literal_doc(doc2);

            if doc1_is_lit && doc2_is_lit {
                handle_literal(doc1, state, printer);
                handle_literal(doc2, state, printer);
            } else if doc1_is_lit && !doc2_is_lit {
                handle_literal(doc1, state, printer);

                state.stack.push(PrintItem::new(doc2, state.indent_delta));
            } else {
                state.stack.push(PrintItem::new(doc2, state.indent_delta));
                state.stack.push(PrintItem::new(doc1, state.indent_delta));
            }
        }

        Doc::TripleDoc(doc1, doc2, doc3) => {
            let doc3_is_lit = is_literal_doc(doc3);
            let doc2_is_lit = is_literal_doc(doc2);
            let doc1_is_lit = is_literal_doc(doc1);

            if doc1_is_lit && doc2_is_lit && doc3_is_lit {
                handle_literal(doc1, state, printer);
                handle_literal(doc2, state, printer);
                handle_literal(doc3, state, printer);
            } else if doc1_is_lit && doc2_is_lit && !doc3_is_lit {
                handle_literal(doc1, state, printer);
                handle_literal(doc2, state, printer);

                state.stack.push(PrintItem::new(doc3, state.indent_delta));
            } else if doc1_is_lit && !doc2_is_lit && !doc3_is_lit {
                handle_literal(doc1, state, printer);

                state.stack.push(PrintItem::new(doc3, state.indent_delta));
                state.stack.push(PrintItem::new(doc2, state.indent_delta));
            } else {
                state.stack.push(PrintItem::new(doc3, state.indent_delta));
                state.stack.push(PrintItem::new(doc2, state.indent_delta));
                state.stack.push(PrintItem::new(doc1, state.indent_delta));
            }
        }
        _ => {
            unreachable!()
        }
    }
}

/// Core pretty printing function.
///
/// Takes a document and a printer configuration and returns a String.
/// Uses a stack to avoid recursion, keeping track of the current line length,
/// and indent level.
pub fn pprint<'a>(doc: impl Into<Doc<'a>>, mut printer: Printer) -> String {
    let doc = doc.into();

    let mut text_length_cache = FxHashMap::default();
    let estimated_output = count_text_length(&doc, &printer, &mut text_length_cache);

    let mut state = PrintState {
        stack: Vec::with_capacity(64),
        output: Vec::with_capacity(estimated_output.max(1024)),

        current_line_len: 0,
        indent_delta: 0,

        space_cache: Vec::with_capacity(128),
        join_breaks: Vec::new(),
        doc_lengths: Vec::new(),
        text_length_cache,
    };

    state.stack.push(PrintItem {
        doc: &doc,
        indent_delta: 0,
        left: None,
        break_left: 0,
        break_mode: false,
    });

    while let Some(PrintItem {
        doc,
        indent_delta,
        left,
        break_left,
        break_mode,
    }) = state.stack.pop()
    {
        if let Some(left) = left {
            handle_literal(left, &mut state, &mut printer);
        }
        if break_left > 0 {
            // Strip trailing whitespace before the line break.
            while state.output.last() == Some(&b' ') || state.output.last() == Some(&b'\t') {
                state.output.pop();
            }
            state.current_line_len = append_line(&mut state, &mut printer);
        }

        let (doc, indent_delta) = match doc {
            Doc::Indent(d) => (d.as_ref(), indent_delta.saturating_add(printer.indent)),
            Doc::Dedent(d) => (d.as_ref(), indent_delta.saturating_sub(printer.indent)),
            _ => (doc, indent_delta),
        };

        state.indent_delta = indent_delta;

        match doc {
            Doc::Concat(docs) => {
                for d in docs.iter().rev() {
                    state.stack.push(PrintItem {
                        doc: d,
                        indent_delta,
                        left: None,
                        break_left: 0,
                        break_mode,
                    });
                }
            }
            Doc::Group(d) => {
                let group_width = count_text_length(d, &printer, &mut state.text_length_cache);
                let needs_breaking =
                    state.current_line_len.saturating_add(group_width) > printer.max_width;
                // Standard Wadler-Lindig: Group only sets break_mode for children.
                // IfBreak docs inside the Group handle actual line breaking.
                // No automatic leading break or trailing Hardline.
                state.stack.push(PrintItem {
                    doc: d,
                    indent_delta,
                    left: None,
                    break_left: 0,
                    break_mode: needs_breaking,
                });
            }
            Doc::IfBreak(doc, other) => {
                let doc = if break_mode { doc } else { other };
                state.stack.push(PrintItem {
                    doc,
                    indent_delta,
                    left: None,
                    break_left: 0,
                    break_mode,
                });
            }
            Doc::Join(inner) | Doc::SmartJoin(inner) => {
                handle_join(doc, &inner.0, &inner.1, &mut state, &mut printer);
            }

            Doc::DoubleDoc(_, _) | Doc::TripleDoc(_, _, _) => {
                handle_n_docs_unrolled(doc, &mut state, &mut printer);
            }
            Doc::Indent(_) | Doc::Dedent(_) => {
                unreachable!("Indent/Dedent should be normalized before dispatch");
            }
            Doc::Null
            | Doc::Char(_)
            | Doc::DoubleChar(_)
            | Doc::TripleChar(_)
            | Doc::QuadChar(_)
            | Doc::SmallBytes(_, _)
            | Doc::Bytes(_, _)
            | Doc::String(_)
            | Doc::i8(_)
            | Doc::i16(_)
            | Doc::i32(_)
            | Doc::i64(_)
            | Doc::i128(_)
            | Doc::isize(_)
            | Doc::u8(_)
            | Doc::u16(_)
            | Doc::u32(_)
            | Doc::u64(_)
            | Doc::u128(_)
            | Doc::usize(_)
            | Doc::f32(_)
            | Doc::f64(_)
            | Doc::Line
            | Doc::Softline
            | Doc::Mediumline
            | Doc::Hardline => {
                handle_literal(doc, &mut state, &mut printer);
            }
        }
    }

    String::from_utf8(state.output).expect(
        "pprint: output buffer contained invalid UTF-8 — all Doc sources must produce valid UTF-8",
    )
}

/// Pretty-print a document by reference, avoiding cloning.
///
/// Same as `pprint()` but borrows the Doc tree instead of consuming it.
/// Useful for benchmarks and when the same Doc tree needs to be rendered
/// multiple times (e.g., LSP formatting).
pub fn pprint_ref<'a>(doc: &'a Doc<'a>, mut printer: Printer) -> String {
    let mut text_length_cache = FxHashMap::default();
    let estimated_output = count_text_length(doc, &printer, &mut text_length_cache);

    let mut state = PrintState {
        stack: Vec::with_capacity(64),
        output: Vec::with_capacity(estimated_output.max(1024)),

        current_line_len: 0,
        indent_delta: 0,

        space_cache: Vec::with_capacity(128),
        join_breaks: Vec::new(),
        doc_lengths: Vec::new(),
        text_length_cache,
    };

    state.stack.push(PrintItem {
        doc,
        indent_delta: 0,
        left: None,
        break_left: 0,
        break_mode: false,
    });

    while let Some(PrintItem {
        doc,
        indent_delta,
        left,
        break_left,
        break_mode,
    }) = state.stack.pop()
    {
        if let Some(left) = left {
            handle_literal(left, &mut state, &mut printer);
        }
        if break_left > 0 {
            while state.output.last() == Some(&b' ') || state.output.last() == Some(&b'\t') {
                state.output.pop();
            }
            state.current_line_len = append_line(&mut state, &mut printer);
        }

        let (doc, indent_delta) = match doc {
            Doc::Indent(d) => (d.as_ref(), indent_delta.saturating_add(printer.indent)),
            Doc::Dedent(d) => (d.as_ref(), indent_delta.saturating_sub(printer.indent)),
            _ => (doc, indent_delta),
        };

        state.indent_delta = indent_delta;

        match doc {
            Doc::Concat(docs) => {
                for d in docs.iter().rev() {
                    state.stack.push(PrintItem {
                        doc: d,
                        indent_delta,
                        left: None,
                        break_left: 0,
                        break_mode,
                    });
                }
            }
            Doc::Group(d) => {
                let group_width = count_text_length(d, &printer, &mut state.text_length_cache);
                let needs_breaking =
                    state.current_line_len.saturating_add(group_width) > printer.max_width;
                state.stack.push(PrintItem {
                    doc: d,
                    indent_delta,
                    left: None,
                    break_left: 0,
                    break_mode: needs_breaking,
                });
            }
            Doc::IfBreak(doc, other) => {
                let doc = if break_mode { doc } else { other };
                state.stack.push(PrintItem {
                    doc,
                    indent_delta,
                    left: None,
                    break_left: 0,
                    break_mode,
                });
            }
            Doc::Join(inner) | Doc::SmartJoin(inner) => {
                handle_join(doc, &inner.0, &inner.1, &mut state, &mut printer);
            }

            Doc::DoubleDoc(_, _) | Doc::TripleDoc(_, _, _) => {
                handle_n_docs_unrolled(doc, &mut state, &mut printer);
            }
            Doc::Indent(_) | Doc::Dedent(_) => {
                unreachable!("Indent/Dedent should be normalized before dispatch");
            }
            Doc::Null
            | Doc::Char(_)
            | Doc::DoubleChar(_)
            | Doc::TripleChar(_)
            | Doc::QuadChar(_)
            | Doc::SmallBytes(_, _)
            | Doc::Bytes(_, _)
            | Doc::String(_)
            | Doc::i8(_)
            | Doc::i16(_)
            | Doc::i32(_)
            | Doc::i64(_)
            | Doc::i128(_)
            | Doc::isize(_)
            | Doc::u8(_)
            | Doc::u16(_)
            | Doc::u32(_)
            | Doc::u64(_)
            | Doc::u128(_)
            | Doc::usize(_)
            | Doc::f32(_)
            | Doc::f64(_)
            | Doc::Line
            | Doc::Softline
            | Doc::Mediumline
            | Doc::Hardline => {
                handle_literal(doc, &mut state, &mut printer);
            }
        }
    }

    String::from_utf8(state.output).expect(
        "pprint: output buffer contained invalid UTF-8 — all Doc sources must produce valid UTF-8",
    )
}

#[derive(Debug, Clone, Copy)]
pub struct Printer {
    pub max_width: usize,
    pub indent: usize,
    pub use_tabs: bool,
}

/// Default printer configuration.
pub const PRINTER: Printer = Printer {
    max_width: 80,
    indent: 4,
    use_tabs: false,
};

impl Default for Printer {
    fn default() -> Self {
        PRINTER
    }
}

/// A builder for a printer configuration.
/// Allows for setting the max width, indent, and whether to use tabs.
impl Printer {
    pub const fn new(max_width: usize, indent: usize, use_tabs: bool) -> Self {
        Printer {
            max_width,
            indent,
            use_tabs,
        }
    }
}
