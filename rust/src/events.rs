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
    /// Begin a Join scope. Separator events follow, then JoinSepEnd, then items
    /// wrapped in JoinItemStart/JoinItemEnd pairs.
    /// `JoinKind` controls break behavior: Plain inherits parent break_mode,
    /// Smart runs text_justify, Linear does inline forward scan.
    JoinStart(JoinKind),
    /// End of separator events within a Join.
    JoinSepEnd,
    /// Begin a join item. `u32` = flat width of this item's events.
    JoinItemStart(u32),
    /// End of a join item.
    JoinItemEnd,
    /// End of Join scope.
    JoinEnd,
}

/// Join break-decision strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// Plain join: items inherit break_mode from enclosing Group.
    Plain,
    /// Smart join: text_justify pre-pass determines break positions.
    Smart,
    /// Linear join: inline forward scan, break when line overflows.
    Linear,
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

        Doc::Join(inner) => {
            emit_join(JoinKind::Plain, &inner.0, &inner.1, events);
        }
        Doc::SmartJoin(inner) => {
            emit_join(JoinKind::Smart, &inner.0, &inner.1, events);
        }
        Doc::LinearJoin(inner) => {
            emit_join(JoinKind::Linear, &inner.0, &inner.1, events);
        }
    }
}

/// Emit structured Join events: JoinStart, separator, then items with widths.
fn emit_join<'a>(
    kind: JoinKind,
    sep: &'a crate::doc::Doc<'a>,
    items: &'a [crate::doc::Doc<'a>],
    events: &mut DocEvents<'a>,
) {
    events.push(DocEvent::JoinStart(kind));
    // Emit separator events.
    doc_to_events(sep, events);
    events.push(DocEvent::JoinSepEnd);
    // Emit each item with its flat width.
    for item in items {
        // Compute this item's flat width by emitting to a temporary buffer.
        let width_start = events.len();
        events.push(DocEvent::JoinItemStart(0)); // placeholder
        doc_to_events(item, events);
        let width_end = events.len();
        // Compute flat width of the item's events.
        let item_width = compute_flat_width(&events.events[width_start + 1..width_end]);
        events.events[width_start] = DocEvent::JoinItemStart(item_width as u32);
        events.push(DocEvent::JoinItemEnd);
    }
    events.push(DocEvent::JoinEnd);
}

fn push_formatted<'a, T: std::fmt::Display>(events: &mut DocEvents<'a>, value: &T) {
    let s = format!("{}", value);
    let leaked: &'a str = Box::leak(s.into_boxed_str());
    events.push(DocEvent::Text(leaked, leaked.len() as u32));
}

/// Render an event buffer to a String.
/// State for an active Join being rendered.
struct JoinState {
    kind: JoinKind,
    sep_start: usize,
    sep_end: usize,
    breaks: Vec<usize>,
    current_item: usize,
    parent_break: bool,
}

/// Pending line break from a Join separator. Deferred until after the item's
/// leading structural events (Indent, GroupStart) so the newline uses the
/// correct indent level.
#[derive(Clone, Copy)]
enum PendingBreak {
    None,
    /// Emit a newline (indent will be read at emission time).
    Deferred,
}

pub fn render_events<'a>(events: &DocEvents<'a>, printer: &Printer) -> String {
    let evs = &events.events;
    let mut output: Vec<u8> = Vec::with_capacity(evs.len() * 4);
    let mut i = 0usize;
    let mut indent = 0usize;
    let mut line_len = 0usize;
    let mut break_stack: Vec<bool> = Vec::with_capacity(32);
    let mut join_stack: Vec<JoinState> = Vec::new();
    let mut pending_break = PendingBreak::None;

    while i < evs.len() {
        match evs[i] {
            DocEvent::Text(s, len) => {
                if matches!(pending_break, PendingBreak::Deferred) {
                    strip_trailing_whitespace(&mut output);
                    emit_newline(&mut output, indent);
                    line_len = indent;
                    pending_break = PendingBreak::None;
                }
                output.extend_from_slice(s.as_bytes());
                line_len += len as usize;
            }
            DocEvent::Char(b) => {
                if matches!(pending_break, PendingBreak::Deferred) {
                    strip_trailing_whitespace(&mut output);
                    emit_newline(&mut output, indent);
                    line_len = indent;
                    pending_break = PendingBreak::None;
                }
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

            DocEvent::JoinStart(kind) => {
                i += 1;
                // Collect separator range.
                let sep_start = i;
                while i < evs.len() && !matches!(evs[i], DocEvent::JoinSepEnd) { i += 1; }
                let sep_end = i;
                let sep_width = compute_flat_width(&evs[sep_start..sep_end]);
                i += 1; // skip JoinSepEnd

                // Pre-scan: collect item widths from JoinItemStart markers.
                let items_scan_start = i;
                let mut item_widths: Vec<usize> = Vec::new();
                {
                    let mut j = i;
                    while j < evs.len() && !matches!(evs[j], DocEvent::JoinEnd) {
                        if let DocEvent::JoinItemStart(w) = evs[j] {
                            item_widths.push(w as usize);
                        }
                        j += 1;
                    }
                }

                // Compute break positions.
                let parent_break = break_stack.last().copied().unwrap_or(false);
                let mut join_breaks: Vec<usize> = Vec::new();
                match kind {
                    JoinKind::Smart => {
                        let max_w = printer.max_width.saturating_sub(indent);
                        crate::utils::text_justify(
                            sep_width, &item_widths, max_w,
                            line_len.saturating_sub(indent), &mut join_breaks,
                        );
                    }
                    JoinKind::Linear => {
                        let max_w = printer.max_width.saturating_sub(indent);
                        let mut ll = line_len;
                        for (idx, &w) in item_widths.iter().enumerate() {
                            if idx > 0 {
                                if ll + sep_width + w > max_w {
                                    join_breaks.push(idx);
                                    ll = indent + w;
                                } else {
                                    ll += sep_width + w;
                                }
                            } else {
                                ll += w;
                            }
                        }
                    }
                    JoinKind::Plain => {
                        if parent_break {
                            for idx in 1..item_widths.len() { join_breaks.push(idx); }
                        }
                    }
                }

                // Push join state for the main loop to use when processing items.
                join_stack.push(JoinState {
                    kind,
                    sep_start,
                    sep_end,
                    breaks: join_breaks,
                    current_item: 0,
                    parent_break,
                });
                // Don't let the outer i += 1 skip the first JoinItemStart.
                continue;
            }

            DocEvent::JoinSepEnd => {
                // Skip — separator was already captured during JoinStart.
            }

            DocEvent::JoinItemStart(_) => {
                if let Some(js) = join_stack.last_mut() {
                    let idx = js.current_item;
                    let should_break = js.breaks.contains(&idx);

                    // Check if separator contains a Hardline (unconditional break).
                    let sep_has_hardline = (js.sep_start..js.sep_end)
                        .any(|si| matches!(evs[si], DocEvent::Hardline));

                    // Emit separator before non-first items.
                    if idx > 0 {
                        let item_bm = match js.kind {
                            JoinKind::Smart => should_break,
                            JoinKind::Linear => false,
                            JoinKind::Plain => js.parent_break,
                        };

                        if sep_has_hardline || should_break {
                            // Emit break immediately at the CURRENT (parent's) indent.
                            // The item's own Indent events will adjust indent for its content.
                            strip_trailing_whitespace(&mut output);
                            emit_newline(&mut output, indent);
                            line_len = indent;
                        } else {
                            // Emit separator content inline (non-Hardline separators like ", ").
                            break_stack.push(item_bm);
                            for si in js.sep_start..js.sep_end {
                                match evs[si] {
                                    DocEvent::Text(s, len) => {
                                        output.extend_from_slice(s.as_bytes());
                                        line_len += len as usize;
                                    }
                                    DocEvent::Char(b) => { output.push(b); line_len += 1; }
                                    DocEvent::Softline => {
                                        if item_bm {
                                            strip_trailing_whitespace(&mut output);
                                            emit_newline(&mut output, indent);
                                            line_len = indent;
                                        } else {
                                            output.push(b' ');
                                            line_len += 1;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            break_stack.pop();
                        }
                    }

                    // Set break_mode for item content.
                    let item_bm = match js.kind {
                        JoinKind::Smart => should_break,
                        JoinKind::Linear => false,
                        JoinKind::Plain => js.parent_break,
                    };
                    break_stack.push(item_bm);

                    js.current_item += 1;
                }
            }

            DocEvent::JoinItemEnd => {
                // Pop the item's break_mode.
                break_stack.pop();
            }

            DocEvent::JoinEnd => {
                join_stack.pop();
            }
        }
        i += 1;
    }

    // Strip trailing whitespace from final line.
    strip_trailing_whitespace(&mut output);

    unsafe { String::from_utf8_unchecked(output) }
}

/// Render a slice of events into the output, reusing the current rendering state.
/// Used by Join rendering to process separator and item events inline.
fn render_event_slice(
    _all_events: &[DocEvent],
    slice: &[DocEvent],
    output: &mut Vec<u8>,
    line_len: &mut usize,
    mut indent: usize,
    break_stack: &mut Vec<bool>,
    printer: &Printer,
) {
    let mut i = 0;
    while i < slice.len() {
        match slice[i] {
            DocEvent::Text(s, len) => {
                output.extend_from_slice(s.as_bytes());
                *line_len += len as usize;
            }
            DocEvent::Char(b) => {
                output.push(b);
                *line_len += 1;
            }
            DocEvent::Hardline => {
                strip_trailing_whitespace(output);
                emit_newline(output, indent);
                *line_len = indent;
            }
            DocEvent::Softline => {
                if break_stack.last().copied().unwrap_or(false) {
                    strip_trailing_whitespace(output);
                    emit_newline(output, indent);
                    *line_len = indent;
                } else {
                    output.push(b' ');
                    *line_len += 1;
                }
            }
            DocEvent::GroupStart(offset) => {
                let end = (i + 1 + offset as usize).min(slice.len());
                let flat_width = compute_flat_width(&slice[i + 1..end]);
                let breaks = flat_width == usize::MAX
                    || line_len.saturating_add(flat_width) > printer.max_width;
                break_stack.push(breaks);
            }
            DocEvent::GroupEnd => {
                break_stack.pop();
            }
            DocEvent::Indent => indent += printer.indent,
            DocEvent::Dedent => indent = indent.saturating_sub(printer.indent),
            DocEvent::IfBreak(break_len) => {
                if break_stack.last().copied().unwrap_or(false) {
                    i += 1;
                    continue;
                } else {
                    i += 1 + break_len as usize + 1;
                    continue;
                }
            }
            DocEvent::IfElse => {
                i = skip_to_if_end(slice, i + 1);
                continue;
            }
            DocEvent::IfEnd => {}
            // Join events inside a separator or item — shouldn't happen in well-formed buffers.
            _ => {}
        }
        i += 1;
    }
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
            // Join in flat mode: sum all item widths + separators.
            DocEvent::JoinStart(_) => {
                i += 1;
                // Measure separator width.
                let sep_start = i;
                while i < events.len() && !matches!(events[i], DocEvent::JoinSepEnd) { i += 1; }
                let sep_w = compute_flat_width(&events[sep_start..i]);
                i += 1; // skip JoinSepEnd
                // Sum item widths + (n-1) separator widths.
                let mut n = 0usize;
                while i < events.len() && !matches!(events[i], DocEvent::JoinEnd) {
                    if let DocEvent::JoinItemStart(w) = events[i] {
                        width += w as usize;
                        n += 1;
                        // Skip to JoinItemEnd.
                        i += 1;
                        let mut depth = 1u32;
                        while i < events.len() && depth > 0 {
                            match events[i] {
                                DocEvent::JoinItemStart(_) => depth += 1,
                                DocEvent::JoinItemEnd => depth -= 1,
                                _ => {}
                            }
                            if depth > 0 { i += 1; }
                        }
                        i += 1; // past JoinItemEnd
                    } else {
                        i += 1;
                    }
                }
                if n > 1 { width += sep_w * (n - 1); }
                // i at JoinEnd, will be advanced by outer i += 1
            }
            DocEvent::JoinSepEnd | DocEvent::JoinItemStart(_) | DocEvent::JoinItemEnd | DocEvent::JoinEnd => {}
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
