/// Greedy text justification: packs items into lines, breaking when the next
/// item would exceed `max_width`. Returns break indices in `output`.
///
/// `first_line_offset` accounts for content already on the first line
/// (e.g., `LET(` = 4 chars before the first arg). Subsequent lines start
/// fresh at `max_width`.
#[inline]
pub fn text_justify(
    sep_length: usize,
    doc_lengths: &[usize],
    max_width: usize,
    first_line_offset: usize,
    output: &mut Vec<usize>,
) {
    let n = doc_lengths.len();
    let mut i = 0;
    let mut is_first_line = true;
    while i < n {
        let effective_width = if is_first_line {
            max_width.saturating_sub(first_line_offset)
        } else {
            max_width
        };
        let mut line_length = doc_lengths[i];
        let mut j = i + 1;
        while j < n {
            let next = line_length.saturating_add(sep_length).saturating_add(doc_lengths[j]);
            if next > effective_width {
                break;
            }
            line_length = next;
            j += 1;
        }
        // Only push actual break positions (indices of items that start new lines).
        // Don't push past-the-end sentinels — handle_join's cursor logic requires
        // all break positions to match real item indices.
        if j < n {
            output.push(j);
        }
        is_first_line = false;
        i = j;
    }
}
