//! Event-streaming renderer for pprint.
//!
//! An alternative to the tree-based Doc renderer that processes a flat
//! `Vec<DocEvent>` instead of a heap-allocated Doc tree. Events are pushed
//! sequentially during construction and processed linearly during rendering.
//! No Box/Vec allocations per node, no pointer chasing, sequential memory access.
//!
//! The renderer uses prettier's "fits" algorithm: on GroupStart, scan forward
//! in the event buffer to compute flat width. If it fits, render inline;
//! if not, set break_mode for the Group's scope.

use crate::print::Printer;

/// A formatting event in the flat event buffer.
///
/// All variants are `Copy`—the entire buffer is one `Vec<DocEvent>` with
/// no per-event heap allocation. Text borrows from the source input.
#[derive(Debug, Clone, Copy)]
pub enum DocEvent<'a> {
    /// Borrowed text with pre-computed byte length.
    Text(&'a str, u32),
    /// Single ASCII byte (common for delimiters: `,`, `:`, `;`, `{`, `}`).
    Char(u8),
    /// Unconditional line break.
    Hardline,
    /// Line break if enclosing Group breaks, single space otherwise.
    Softline,
    /// Begin a Group scope. `u32` = offset (in events) to matching GroupEnd.
    GroupStart(u32),
    /// End a Group scope.
    GroupEnd,
    /// Increase indent by `printer.indent`.
    Indent,
    /// Decrease indent by `printer.indent`.
    Dedent,
    /// Conditional: break branch follows, then IfElse, then fit branch, then IfEnd.
    /// `u32` = number of events in the break branch (skip count for fit mode).
    IfBreak(u32),
    /// Boundary between break and fit branches inside an IfBreak.
    IfElse,
    /// End of conditional.
    IfEnd,
}

/// A flat event buffer for streaming pretty-printing.
pub struct DocEvents<'a> {
    pub events: Vec<DocEvent<'a>>,
}

impl<'a> DocEvents<'a> {
    pub fn new() -> Self {
        Self {
            events: Vec::with_capacity(1024),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: Vec::with_capacity(cap),
        }
    }

    #[inline(always)]
    pub fn push(&mut self, event: DocEvent<'a>) {
        self.events.push(event);
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Push a GroupStart placeholder and return the index for later patching.
    #[inline(always)]
    pub fn open_group(&mut self) -> usize {
        let idx = self.events.len();
        self.events.push(DocEvent::GroupStart(0));
        idx
    }

    /// Close a Group by patching the GroupStart at `start_idx` with the offset
    /// to the GroupEnd event.
    #[inline(always)]
    pub fn close_group(&mut self, start_idx: usize) {
        let offset = (self.events.len() - start_idx) as u32;
        self.events[start_idx] = DocEvent::GroupStart(offset);
        self.events.push(DocEvent::GroupEnd);
    }

    /// Push an IfBreak placeholder and return the index for later patching.
    #[inline(always)]
    pub fn open_if_break(&mut self) -> usize {
        let idx = self.events.len();
        self.events.push(DocEvent::IfBreak(0));
        idx
    }

    /// Mark the IfElse boundary and patch the IfBreak at `start_idx`.
    #[inline(always)]
    pub fn mark_if_else(&mut self, start_idx: usize) {
        let break_len = (self.events.len() - start_idx - 1) as u32;
        self.events[start_idx] = DocEvent::IfBreak(break_len);
        self.events.push(DocEvent::IfElse);
    }

    /// Close an IfBreak conditional.
    #[inline(always)]
    pub fn close_if_break(&mut self) {
        self.events.push(DocEvent::IfEnd);
    }
}

/// Convert a Doc tree into a flat event buffer.
///
/// This is the bridge from the existing tree-based API to the event renderer.
/// Once validated, grammars can emit events directly via to_events() codegen,
/// skipping the Doc tree entirely.
pub fn doc_to_events<'a>(doc: &'a crate::doc::Doc<'a>, events: &mut DocEvents<'a>) {
    use crate::doc::Doc;
    match doc {
        Doc::Null => {}
        Doc::Char(b) => events.push(DocEvent::Char(*b)),
        Doc::DoubleChar(bs) => {
            events.push(DocEvent::Char(bs[0]));
            events.push(DocEvent::Char(bs[1]));
        }
        Doc::TripleChar(bs) => {
            events.push(DocEvent::Char(bs[0]));
            events.push(DocEvent::Char(bs[1]));
            events.push(DocEvent::Char(bs[2]));
        }
        Doc::QuadChar(bs) => {
            events.push(DocEvent::Char(bs[0]));
            events.push(DocEvent::Char(bs[1]));
            events.push(DocEvent::Char(bs[2]));
            events.push(DocEvent::Char(bs[3]));
        }
        Doc::SmallBytes(bs, len) => {
            let s = unsafe { std::str::from_utf8_unchecked(&bs[..*len]) };
            events.push(DocEvent::Text(s, *len as u32));
        }
        Doc::Bytes(bs, len) => {
            let s = unsafe { std::str::from_utf8_unchecked(&bs[..*len]) };
            events.push(DocEvent::Text(s, *len as u32));
        }
        Doc::String(cow) => {
            // String(Cow) can't produce a borrowed &'a str in all cases.
            // For Cow::Borrowed, the lifetime works. For Cow::Owned, we'd
            // need to leak or use a different strategy. For now, handle
            // borrowed case and fall back for owned.
            match cow {
                std::borrow::Cow::Borrowed(s) => {
                    events.push(DocEvent::Text(s, s.len() as u32));
                }
                std::borrow::Cow::Owned(s) => {
                    // Owned strings can't be borrowed into the event buffer
                    // without leaking. Use Text with a leaked reference.
                    // This is acceptable for a prototype benchmark.
                    let leaked: &'a str = Box::leak(s.clone().into_boxed_str());
                    events.push(DocEvent::Text(leaked, leaked.len() as u32));
                }
            }
        }
        Doc::Text(s, len) => events.push(DocEvent::Text(s, *len)),

        // Numeric types: format to stack buffer, leak for borrowed lifetime.
        Doc::i8(v) => push_formatted(events, v),
        Doc::i16(v) => push_formatted(events, v),
        Doc::i32(v) => push_formatted(events, v),
        Doc::i64(v) => push_formatted(events, v),
        Doc::i128(v) => push_formatted(events, v),
        Doc::isize(v) => push_formatted(events, v),
        Doc::u8(v) => push_formatted(events, v),
        Doc::u16(v) => push_formatted(events, v),
        Doc::u32(v) => push_formatted(events, v),
        Doc::u64(v) => push_formatted(events, v),
        Doc::u128(v) => push_formatted(events, v),
        Doc::usize(v) => push_formatted(events, v),
        Doc::f32(v) => push_formatted(events, v),
        Doc::f64(v) => push_formatted(events, v),

        Doc::Hardline => events.push(DocEvent::Hardline),
        Doc::Softline => events.push(DocEvent::Softline),
        Doc::Mediumline => {} // zero-width in flat mode
        Doc::Line => events.push(DocEvent::Hardline),

        Doc::Concat(children) => {
            for child in children {
                doc_to_events(child, events);
            }
        }
        Doc::DoubleDoc(a, b) => {
            doc_to_events(a, events);
            doc_to_events(b, events);
        }
        Doc::TripleDoc(a, b, c) => {
            doc_to_events(a, events);
            doc_to_events(b, events);
            doc_to_events(c, events);
        }

        Doc::Group(inner) => {
            let start = events.open_group();
            doc_to_events(inner, events);
            events.close_group(start);
        }

        Doc::Indent(inner) => {
            events.push(DocEvent::Indent);
            doc_to_events(inner, events);
            events.push(DocEvent::Dedent);
        }
        Doc::Dedent(inner) => {
            events.push(DocEvent::Dedent);
            doc_to_events(inner, events);
            events.push(DocEvent::Indent);
        }

        Doc::IfBreak(break_doc, fit_doc) => {
            let start = events.open_if_break();
            doc_to_events(break_doc, events);
            events.mark_if_else(start);
            doc_to_events(fit_doc, events);
            events.close_if_break();
        }

        Doc::Join(inner) | Doc::SmartJoin(inner) | Doc::LinearJoin(inner) => {
            let (sep, items) = inner.as_ref();
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    doc_to_events(sep, events);
                }
                doc_to_events(item, events);
            }
        }
    }
}

fn push_formatted<'a, T: std::fmt::Display>(events: &mut DocEvents<'a>, value: &T) {
    let s = format!("{}", value);
    let leaked: &'a str = Box::leak(s.into_boxed_str());
    events.push(DocEvent::Text(leaked, leaked.len() as u32));
}

/// Render an event buffer to a String.
pub fn render_events<'a>(events: &DocEvents<'a>, printer: &Printer) -> String {
    let evs = &events.events;
    let mut output: Vec<u8> = Vec::with_capacity(evs.len() * 4);
    let mut i = 0usize;
    let mut indent = 0usize;
    let mut line_len = 0usize;
    let mut break_stack: Vec<bool> = Vec::with_capacity(32);

    while i < evs.len() {
        match evs[i] {
            DocEvent::Text(s, len) => {
                output.extend_from_slice(s.as_bytes());
                line_len += len as usize;
            }
            DocEvent::Char(b) => {
                output.push(b);
                line_len += 1;
            }
            DocEvent::Hardline => {
                strip_trailing_whitespace(&mut output);
                emit_newline(&mut output, indent);
                line_len = indent;
            }
            DocEvent::Softline => {
                if break_stack.last().copied().unwrap_or(false) {
                    strip_trailing_whitespace(&mut output);
                    emit_newline(&mut output, indent);
                    line_len = indent;
                } else {
                    output.push(b' ');
                    line_len += 1;
                }
            }
            DocEvent::GroupStart(offset) => {
                let end = (i + 1 + offset as usize).min(evs.len());
                let flat_width = compute_flat_width(&evs[i + 1..end]);
                let breaks = flat_width == usize::MAX
                    || line_len.saturating_add(flat_width) > printer.max_width;
                break_stack.push(breaks);
            }
            DocEvent::GroupEnd => {
                break_stack.pop();
            }
            DocEvent::Indent => {
                indent += printer.indent;
            }
            DocEvent::Dedent => {
                indent = indent.saturating_sub(printer.indent);
            }
            DocEvent::IfBreak(break_len) => {
                if break_stack.last().copied().unwrap_or(false) {
                    // Use break branch: advance past IfBreak, render break events.
                    i += 1;
                    continue;
                } else {
                    // Use fit branch: skip break branch + IfElse marker.
                    i += 1 + break_len as usize + 1;
                    continue;
                }
            }
            DocEvent::IfElse => {
                // Finished rendering break branch. Skip the fit branch to IfEnd.
                i = skip_to_if_end(evs, i + 1);
                continue;
            }
            DocEvent::IfEnd => {
                // End of conditional, continue.
            }
        }
        i += 1;
    }

    // Strip trailing whitespace from final line.
    strip_trailing_whitespace(&mut output);

    unsafe { String::from_utf8_unchecked(output) }
}

/// Compute the flat (single-line) width of an event slice.
/// Returns `usize::MAX` if a Hardline is encountered (forces Group break).
fn compute_flat_width(events: &[DocEvent]) -> usize {
    let mut width = 0usize;
    let mut i = 0;
    while i < events.len() {
        match events[i] {
            DocEvent::Text(_, len) => width += len as usize,
            DocEvent::Char(_) => width += 1,
            DocEvent::Softline => width += 1,
            DocEvent::Hardline => return usize::MAX,
            DocEvent::GroupStart(_) | DocEvent::GroupEnd => {}
            DocEvent::Indent | DocEvent::Dedent => {}
            DocEvent::IfBreak(break_len) => {
                // In flat mode, use the fit branch. Skip break branch + IfElse.
                i += 1 + break_len as usize + 1;
                continue;
            }
            DocEvent::IfElse => {
                // Shouldn't reach here during flat-width scan, but handle gracefully.
                i = skip_to_if_end(events, i + 1);
                continue;
            }
            DocEvent::IfEnd => {}
        }
        i += 1;
    }
    width
}

/// Skip forward to the matching IfEnd, handling nested IfBreak depth.
fn skip_to_if_end(events: &[DocEvent], start: usize) -> usize {
    let mut depth = 1u32;
    let mut i = start;
    while i < events.len() && depth > 0 {
        match events[i] {
            DocEvent::IfBreak(_) => depth += 1,
            DocEvent::IfEnd => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    i
}

#[inline(always)]
fn emit_newline(output: &mut Vec<u8>, indent: usize) {
    output.push(b'\n');
    for _ in 0..indent {
        output.push(b' ');
    }
}

#[inline(always)]
fn strip_trailing_whitespace(output: &mut Vec<u8>) {
    while output.last() == Some(&b' ') || output.last() == Some(&b'\t') {
        output.pop();
    }
}
