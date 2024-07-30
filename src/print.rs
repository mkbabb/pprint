use lazy_static::lazy_static;

use crate::doc::Doc;
use crate::utils::text_justify;
use crate::DigitCount;

use std::collections::HashSet;

use std::io::Write as _;
use std::sync::Mutex;

use std::mem::size_of;

struct PrintItem<'a> {
    doc: &'a Doc<'a>,
    indent_delta: usize,

    left: Option<&'a Doc<'a>>,
    break_left: usize,
}

struct PrintState<'a> {
    stack: Vec<PrintItem<'a>>,
    output: Vec<u8>,

    current_line_len: usize,
    indent_delta: usize,

    space_cache: Vec<u8>,
    join_breaks: Vec<usize>,
}

#[inline(always)]
fn is_literal_doc(doc: &Doc) -> bool {
    fn is_literal(doc: &Doc) -> bool {
        matches!(
            doc,
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
                | Doc::Hardline
        )
    }

    match doc {
        Doc::DoubleDoc(doc1, doc2) => is_literal_doc(doc1) && is_literal_doc(doc2),
        Doc::TripleDoc(doc1, doc2, doc3) => {
            is_literal_doc(doc1) && is_literal_doc(doc2) && is_literal_doc(doc3)
        }
        Doc::HardlineDoc(doc) => is_literal_doc(doc),
        // Doc::Concat(docs) => docs.iter().all(is_literal),
        _ => is_literal(doc),
    }
}

#[inline(always)]
pub fn count_join_length<'a>(sep: &'a Doc<'a>, docs: &'a [Doc<'a>], printer: &Printer) -> usize {
    if docs.is_empty() {
        return 0;
    }

    let doc_len: usize = docs.iter().map(|d| count_text_length(d, printer)).sum();
    let sep_len = count_text_length(sep, printer);

    doc_len + sep_len * (docs.len() - 1)
}

#[inline]
pub fn count_text_length<'a>(doc: &'a Doc<'a>, printer: &Printer) -> usize {
    match doc {
        Doc::Concat(docs) => docs.iter().map(|d| count_text_length(d, printer)).sum(),

        Doc::Group(d) => count_text_length(d, printer),

        Doc::Indent(d) => count_text_length(d, printer).saturating_add(printer.indent),
        Doc::Dedent(d) => count_text_length(d, printer).saturating_sub(printer.indent),

        Doc::Join(sep, docs) => count_join_length(sep, docs, printer),

        Doc::SmartJoin(sep, docs) => {
            let len = count_join_length(sep, docs, printer);
            len.min(len + printer.max_width)
        }

        Doc::IfBreak(t, f) => count_text_length(t, printer).max(count_text_length(f, printer)),

        Doc::Softline => printer.max_width / 2,

        Doc::Hardline | Doc::Line => printer.max_width,

        Doc::Char(_) => 1,
        Doc::DoubleChar(_) => 2,
        Doc::TripleChar(_) => 3,
        Doc::QuadChar(_) => 4,

        Doc::SmallBytes(_, len) => *len,
        Doc::Bytes(_, len) => *len,

        Doc::String(s) => s.len(),

        Doc::i8(value) => value.len(),
        Doc::i16(value) => value.len(),
        Doc::i32(value) => value.len(),
        Doc::i64(value) => value.len(),
        Doc::i128(value) => value.len(),
        Doc::isize(value) => value.len(),

        Doc::u8(value) => value.len(),
        Doc::u16(value) => value.len(),
        Doc::u32(value) => value.len(),
        Doc::u64(value) => value.len(),
        Doc::u128(value) => value.len(),
        Doc::usize(value) => value.len(),

        Doc::f32(_) => 10,
        Doc::f64(_) => 20,

        _ => 0,
    }
}

#[inline(always)]
pub fn smart_join_breaks<'a>(
    sep: &'a Doc<'a>,
    docs: &'a [Doc<'a>],

    state: &mut PrintState<'a>,
    printer: &mut Printer,
) {
    let max_width = (printer.max_width / 4).max(2);

    let sep_length = count_text_length(sep, printer);
    let doc_lengths: Vec<_> = docs.iter().map(|d| count_text_length(d, printer)).collect();

    // Align the separator with the longest doc length:
    let sep_length = sep_length + sep_length.max(doc_lengths.iter().max().copied().unwrap_or(0));

    state.join_breaks.clear();

    text_justify(sep_length, &doc_lengths, max_width, &mut state.join_breaks)
}

#[inline(always)]
fn format_int<T>(value: T, state: &mut PrintState) -> usize
where
    T: itoap::Integer + std::fmt::Display,
{
    // unsafe {
    //     // First, extend the output by i128::MAX_DIGITS (40) bytes:
    //     output.extend_from_slice(&[0; itoa::raw::I128_MAX_LEN]);
    //     // Then, format the integer into the last 40 bytes of the output:
    //     let len = itoa::raw::format(
    //         value,
    //         output
    //             .as_mut_ptr()
    //             .add(output.len() - itoa::raw::I128_MAX_LEN),
    //     );
    //     // Then, truncate the output to the correct length:
    //     output.truncate(output.len() - itoa::raw::I128_MAX_LEN + len);

    //     len
    // }
    let prev_len = state.output.len();
    itoap::write_to_vec(&mut state.output, value);
    state.output.len() - prev_len

    // let mut buf = itoa::Buffer::new();
    // let s = buf.format(value).as_bytes();
    // output.extend_from_slice(s);
    // s.len()

    // let prev_len = output.len();
    // write!(output, "{}", value).unwrap();
    // output.len() - prev_len
}

// Function for f64
#[inline(always)]
fn format_f64<T>(value: T, state: &mut PrintState) -> usize
where
    T: std::fmt::Display + dragonbox::Float,
{
    // unsafe {
    //     const MAX_LEN: usize = 16;
    //     // First, extend the output by 16 bytes:
    //     output.extend_from_slice(&[0; MAX_LEN]);
    //     // Then, format the f32 into the last MAX_LEN bytes of the output:
    //     let len = ryu::raw::format32(value, output.as_mut_ptr().add(output.len() - MAX_LEN));
    //     // Then, truncate the output to the correct length:
    //     output.truncate(output.len() - MAX_LEN + len);

    //     len
    // }

    let mut buf = dragonbox::Buffer::new();
    let s = buf.format_finite(value).as_bytes();
    state.output.extend_from_slice(s);
    s.len()

    // let mut buf = ryu::Buffer::new();
    // let s = buf.format_finite(value).as_bytes();
    // output.extend_from_slice(s);
    // s.len()

    // write!(output, "{}", value as f32).unwrap();
    // output.len()
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

    state.output.extend_from_slice(&space_cache[..indent_delta]);

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

        Doc::Softline if state.current_line_len > printer.max_width => append_line(state, printer),

        _ => state.current_line_len,
    }
}

#[inline(always)]
fn handle_literal<'a>(doc: &'a Doc<'a>, state: &mut PrintState<'a>, printer: &mut Printer) {
    let offset = match doc {
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
            state.output.extend_from_slice(b);
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

        Doc::f32(v) => format_f64(*v as f64, state),
        Doc::f64(v) => format_f64(*v, state),

        _ => 0,
    };

    state.current_line_len = offset + handle_line(doc, state, printer);

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
        _ => {}
    }
}

fn handle_join<'a>(
    doc: &'a Doc<'a>,
    sep: &'a Doc<'a>,
    docs: &'a [Doc<'a>],
    state: &mut PrintState<'a>,
    printer: &mut Printer,
) {
    if let Doc::SmartJoin(_, _) = doc {
        smart_join_breaks(sep, docs, state, printer);
    }

    let sep_is_lit = is_literal_doc(sep);

    for (i, d) in docs.iter().rev().enumerate() {
        let i = docs.len() - i - 1;

        let left = if i > 0 && sep_is_lit { Some(sep) } else { None };

        let break_left = if state.join_breaks.binary_search(&i).is_ok() {
            state.indent_delta
        } else {
            0
        };

        state.stack.push(PrintItem {
            doc: d,
            indent_delta: state.indent_delta,
            left,
            break_left,
        });

        if !sep_is_lit && i > 0 {
            state.stack.push(PrintItem {
                doc: sep,
                indent_delta: state.indent_delta,
                left: None,
                break_left,
            });
        }
    }
}

fn handle_n_docs<'a>(docs: &[&'a Doc<'a>], state: &mut PrintState<'a>, printer: &mut Printer) {
    let mut all_literal = true;
    let mut last_non_literal = docs.len();

    for (i, doc) in docs.iter().enumerate() {
        if !is_literal_doc(doc) {
            all_literal = false;
            last_non_literal = i;
            break;
        }
    }

    if all_literal {
        for doc in docs {
            handle_literal(doc, state, printer);
        }
    } else {
        for (i, doc) in docs.iter().rev().enumerate() {
            let i = docs.len() - i - 1;

            if i < last_non_literal {
                handle_literal(doc, state, printer);
            } else {
                state.stack.push(PrintItem {
                    doc,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });
            }
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

                state.stack.push(PrintItem {
                    doc: doc2,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });
            } else {
                state.stack.push(PrintItem {
                    doc: doc2,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });

                state.stack.push(PrintItem {
                    doc: doc1,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });
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

                state.stack.push(PrintItem {
                    doc: doc3,
                    indent_delta: state.indent_delta,

                    left: None,
                    break_left: 0,
                });
            } else if doc1_is_lit && !doc2_is_lit && !doc3_is_lit {
                handle_literal(doc1, state, printer);

                state.stack.push(PrintItem {
                    doc: doc3,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });

                state.stack.push(PrintItem {
                    doc: doc2,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });
            } else {
                state.stack.push(PrintItem {
                    doc: doc3,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });

                state.stack.push(PrintItem {
                    doc: doc2,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });

                state.stack.push(PrintItem {
                    doc: doc1,
                    indent_delta: state.indent_delta,
                    left: None,
                    break_left: 0,
                });
            }
        }
        _ => {
            unreachable!()
        }
    }
}

/// Core pretty printing function.
///
/// Takes a document and an optional printer configuration and returns a String.
/// Uses a stack to avoid recursion, keeping track of the current line length,
/// and indent level.
pub fn pprint<'a>(doc: impl Into<Doc<'a>>, printer: Option<Printer>) -> String {
    let doc = doc.into();

    let mut printer = printer.unwrap_or_default();

    let mut state = PrintState {
        stack: Vec::with_capacity(64),
        output: Vec::with_capacity(1024),

        current_line_len: 0,
        indent_delta: 0,

        space_cache: Vec::new(),
        join_breaks: Vec::new(),
    };

    state.stack.push(PrintItem {
        doc: &doc,
        indent_delta: 0,
        left: None,
        break_left: 0,
    });

    while let Some(PrintItem {
        doc,
        indent_delta,
        left,
        break_left,
    }) = state.stack.pop()
    {
        if let Some(left) = left {
            handle_literal(left, &mut state, &mut printer);
        }
        if break_left > 0 {
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
                    });
                }
            }
            Doc::Group(d) => {
                let needs_breaking = count_text_length(d, &printer) > printer.max_width;
                if needs_breaking {
                    state.stack.push(PrintItem {
                        doc: &Doc::Hardline,
                        indent_delta: indent_delta.saturating_sub(printer.indent),
                        left: None,
                        break_left: 0,
                    });
                }
                state.stack.push(PrintItem {
                    doc: d,
                    indent_delta,
                    left: None,
                    break_left: if needs_breaking { indent_delta } else { 0 },
                });
            }
            Doc::IfBreak(doc, other) => {
                let is_or_was_broken = state.stack.last().map_or(false, |last| {
                    matches!(last.doc, &Doc::Hardline | &Doc::Softline)
                });
                let doc = if is_or_was_broken { doc } else { other };
                state.stack.push(PrintItem {
                    doc,
                    indent_delta,
                    left: None,
                    break_left: 0,
                });
            }
            Doc::Join(sep, docs) | Doc::SmartJoin(sep, docs) => {
                handle_join(doc, sep, docs, &mut state, &mut printer);
            }

            Doc::DoubleDoc(_, _) | Doc::TripleDoc(_, _, _) => {
                handle_n_docs_unrolled(doc, &mut state, &mut printer);
            }
            _ => {
                handle_literal(doc, &mut state, &mut printer);
            }
        }
    }

    unsafe { String::from_utf8_unchecked(state.output) }
}

#[derive(Debug, Clone)]
pub struct Printer {
    pub max_width: usize,
    pub indent: usize,
    pub break_long_text: bool,
    pub use_tabs: bool,
}

/// Default printer configuration.
pub const PRINTER: Printer = Printer {
    max_width: 80,
    indent: 4,
    break_long_text: false,
    use_tabs: false,
};

impl Default for Printer {
    fn default() -> Self {
        PRINTER.clone()
    }
}

/// A builder for a printer configuration.
/// Allows for setting the max width, indent, whether to break long text,
/// and whether to use tabs.
impl Printer {
    pub const fn new(
        max_width: usize,
        indent: usize,
        break_long_text: bool,
        use_tabs: bool,
    ) -> Self {
        Printer {
            max_width,
            indent,
            break_long_text,
            use_tabs,
        }
    }

    // pub fn pprint<'a>(&self, doc: impl Into<Doc<'a>>) -> String {
    //     pprint(doc.into(), self.clone().into())
    // }
}

// impl std::fmt::Debug for Doc<'_> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let s = PRINTER.pprint(self);
//         f.write_str(&s)
//     }
// }

// impl std::fmt::Display for Doc<'_> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let s = PRINTER.pprint(self);
//         f.write_str(&s)
//     }
// }
