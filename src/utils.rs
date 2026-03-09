/// Greedy text justification: packs items into lines, breaking when the next
/// item would exceed `max_width`. Returns break indices in `output`.
#[inline]
pub fn text_justify(
    sep_length: usize,
    doc_lengths: &[usize],
    max_width: usize,
    output: &mut Vec<usize>,
) {
    let n = doc_lengths.len();
    let mut i = 0;
    while i < n {
        let mut line_length = doc_lengths[i];
        let mut j = i + 1;
        while j < n {
            let next = line_length + sep_length + doc_lengths[j];
            if next > max_width {
                break;
            }
            line_length = next;
            j += 1;
        }
        output.push(j);
        i = j;
    }
}
