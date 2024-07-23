use crate::doc::Doc;
use crate::utils::text_justify;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write as _;
use std::rc::Rc;

pub fn count_join_length<'a>(sep: &'a Doc<'a>, docs: &'a [Doc<'a>], printer: &Printer) -> usize {
    if docs.is_empty() {
        return 0;
    }

    let doc_length: usize = docs.iter().map(|d| count_text_length(d, printer)).sum();
    let sep_length = count_text_length(sep, printer);

    doc_length + sep_length * (docs.len() - 1)
}

pub fn count_text_length<'a>(doc: &'a Doc<'a>, printer: &Printer) -> usize {
    match doc {
        Doc::Char(_) => 1,
        Doc::DoubleChar(_) => 2,
        Doc::TripleChar(_) => 3,
        Doc::QuadChar(_) => 4,

        Doc::SmallBytes(_, len) => *len,
        Doc::Bytes(_, len) => *len,

        Doc::String(s) => s.len(),

        Doc::Concat(docs) => docs.iter().map(|d| count_text_length(d, printer)).sum(),
        Doc::Group(d) => count_text_length(d, printer),
        Doc::Indent(d) => count_text_length(d, printer).saturating_add(printer.indent),
        Doc::Dedent(d) => count_text_length(d, printer).saturating_sub(printer.indent),
        Doc::Join(sep, docs) => count_join_length(sep, docs, printer),
        Doc::IfBreak(t, f) => count_text_length(t, printer).max(count_text_length(f, printer)),
        Doc::SmartJoin(sep, docs) => {
            let length = count_join_length(sep, docs, printer);
            if length * docs.len() >= printer.max_width {
                length + printer.max_width
            } else {
                length
            }
        }
        Doc::Hardline | Doc::Mediumline | Doc::Line => printer.max_width,
        Doc::Softline => printer.max_width / 2,
        _ => 0,
    }
}

pub fn smart_join_breaks<'a>(
    sep: &'a Doc<'a>,
    docs: &'a [Doc<'a>],
    printer: &Printer,
) -> HashSet<usize> {
    let max_width = (printer.max_width / 4).max(2);

    let sep_length = count_text_length(sep, printer);
    let doc_lengths: Vec<_> = docs.iter().map(|d| count_text_length(d, printer)).collect();

    // Align the separator with the longest doc length:
    let sep_length = sep_length + sep_length.max(doc_lengths.iter().max().copied().unwrap_or(0));

    text_justify(sep_length, &doc_lengths, max_width)
}

#[derive(Clone, Copy)]
struct PrintItem<'a> {
    doc: &'a Doc<'a>,
    indent_delta: usize,
    tmp_output_span: Option<(usize, usize)>,
}

fn push_hardline(stack: &mut VecDeque<PrintItem>, indent_delta: usize) {
    stack.push_back(PrintItem {
        doc: &Doc::Hardline,
        indent_delta,
        tmp_output_span: None,
    });
}

fn add_bytes(output: &mut Vec<u8>, b: &[u8]) -> usize {
    output.extend_from_slice(b);
    b.len()
}

fn add_bytes_from_doc<'a>(output: &mut Vec<u8>, doc: &'a Doc<'a>) -> (usize, usize) {
    match doc {
        Doc::Char(c) => {
            output.push(*c);
            (1, 1)
        }
        Doc::DoubleChar(cs) => (add_bytes(output, cs), 2),
        Doc::TripleChar(cs) => (add_bytes(output, cs), 3),
        Doc::QuadChar(cs) => (add_bytes(output, cs), 4),
        Doc::SmallBytes(b, len) => (add_bytes(output, b), *len),
        Doc::Bytes(b, len) => (add_bytes(output, b), *len),
        Doc::String(s) => (add_bytes(output, s.as_bytes()), s.len()),
        _ => (0, 0),
    }
}

fn collapse_bytes_streak<'a, 'b>(
    output: Rc<RefCell<Vec<u8>>>,
    docs: &'a [Doc<'a>],
    indent_delta: usize,
) -> impl DoubleEndedIterator<Item = PrintItem<'a>> + 'a
where
    'b: 'a,
{
    let mut pushed = false;
    let mut streak_len = 0;

    docs.iter()
        .enumerate()
        .filter_map(move |(i, d)| {
            let byte_len = add_bytes_from_doc(&mut output.borrow_mut(), d).0;
            let is_last = i == docs.len() - 1;

            if (byte_len == 0 && pushed) || is_last {
                let output_len = output.borrow().len();
                let tmp_output_span = Some((output_len - streak_len - byte_len, output_len));

                streak_len = 0;
                pushed = false;

                Some(PrintItem {
                    doc: d,
                    indent_delta,
                    tmp_output_span,
                })
            } else if byte_len > 0 {
                streak_len += byte_len;
                pushed = true;

                None
            } else {
                Some(PrintItem {
                    doc: d,
                    indent_delta,
                    tmp_output_span: None,
                })
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
}

/// Core pretty printing function.
/// Takes a document and a printer configuration and returns a String.
/// Uses a stack to avoid recursion, keeping track of the current line length,
/// and indent level.
pub fn pprint<'a>(doc: impl Into<Doc<'a>>, printer: Option<Printer>) -> String {
    let doc = doc.into();

    let printer = printer.unwrap_or_default();

    let mut stack = VecDeque::with_capacity(8);
    // let mut tmp_stack = Vec::with_capacity(8);

    stack.push_back(PrintItem {
        doc: &doc,
        indent_delta: 0,
        tmp_output_span: None,
    });

    let mut output = Vec::new();
    let tmp_output = Rc::new(RefCell::new(Vec::new()));

    let mut prev_len = 0;
    let mut current_line_len = 0;

    let space = (if printer.use_tabs { "\t" } else { " " }).as_bytes();

    let add_hardline = |output: &mut Vec<u8>, indent_delta: usize| {
        let space = space.repeat(indent_delta);

        writeln!(output).unwrap();
        output.extend_from_slice(&space);

        (space.len(), indent_delta)
    };

    while let Some(PrintItem {
        doc,
        indent_delta,
        tmp_output_span,
    }) = stack.pop_back()
    {
        if let Some((start, end)) = tmp_output_span {
            current_line_len += add_bytes(&mut output, &tmp_output.borrow()[start..end]);
        };

        match &doc {
            Doc::Concat(docs) => {
                for d in collapse_bytes_streak(tmp_output.clone(), docs, indent_delta) {
                    stack.push_back(d);
                }

                prev_len = tmp_output.borrow().len();
            }

            Doc::Group(d) => {
                let needs_breaking = count_text_length(d, &printer) > printer.max_width;

                if needs_breaking {
                    push_hardline(&mut stack, indent_delta.saturating_sub(printer.indent));
                }

                stack.push_back(PrintItem {
                    doc: d,
                    indent_delta,
                    tmp_output_span: None,
                });

                if needs_breaking {
                    push_hardline(&mut stack, indent_delta);
                }
            }

            Doc::IfBreak(doc, other) => {
                let mut is_or_was_broken = false;
                if let Some(last) = stack.back() {
                    is_or_was_broken =
                        matches!(last.doc, &Doc::Hardline) || matches!(last.doc, &Doc::Softline);
                }

                let d = if is_or_was_broken { doc } else { other };

                stack.push_back(PrintItem {
                    doc: d,
                    indent_delta,
                    tmp_output_span: None,
                });
            }

            Doc::Indent(d) => {
                stack.push_back(PrintItem {
                    doc: d,
                    indent_delta: indent_delta.saturating_add(printer.indent),
                    tmp_output_span: None,
                });
            }

            Doc::Dedent(d) => {
                stack.push_back(PrintItem {
                    doc: d,
                    indent_delta: indent_delta.saturating_sub(printer.indent),
                    tmp_output_span: None,
                });
            }

            Doc::Join(sep, docs) | Doc::SmartJoin(sep, docs) => {
                let breaks = match doc {
                    Doc::Join(_, _) => None,
                    Doc::SmartJoin(_, _) => Some(smart_join_breaks(sep, docs, &printer)),
                    _ => unreachable!(),
                };

                for (i, d) in docs.iter().rev().enumerate() {
                    let i = docs.len() - i - 1;

                    stack.push_back(PrintItem {
                        doc: d,
                        indent_delta,
                        tmp_output_span: None,
                    });

                    if i > 0 {
                        if let Some(breaks) = &breaks
                            && breaks.contains(&i)
                        {
                            push_hardline(&mut stack, indent_delta);
                        }
                        stack.push_back(PrintItem {
                            doc: sep,
                            indent_delta,
                            tmp_output_span: None,
                        });
                    }
                }
            }

            Doc::Line => {
                current_line_len = 0;
                writeln!(output).unwrap();
            }

            Doc::Hardline => {
                current_line_len = add_hardline(&mut output, indent_delta).0;
            }

            Doc::Mediumline if current_line_len > printer.max_width / 2 => {
                current_line_len = add_hardline(&mut output, indent_delta).0;
            }

            Doc::Softline if current_line_len > printer.max_width => {
                current_line_len = add_hardline(&mut output, indent_delta).0;
            }

            _ if tmp_output_span.is_none() => {
                current_line_len += add_bytes_from_doc(&mut output, doc).0;
            }
            _ => {}
        }
    }

    add_bytes(&mut output, &tmp_output.borrow()[prev_len..]);

    String::from_utf8(output).unwrap()
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
    max_width: 32,
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
