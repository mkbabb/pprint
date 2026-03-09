/// Text justification algorithm inspired by LaTeX's text justification algorithm.
///
/// See: https://en.wikipedia.org/wiki/Line_wrap_and_word_wrap#Minimum_raggedness
/// and: MIT's 6.006, lecture No.20 https://www.youtube.com/watch?v=ENyox7kNKeY
///
/// This function takes a list of document lengths and a maximum line width, and returns a vector
/// of indices that represent the end of each line in the justified text. The algorithm minimizes
/// the "badness" of each line, which is defined as the cube of the unused space at the end of the
/// line.
///
/// # Arguments
///
/// * `sep_length` - The length of the separator between words.
/// * `doc_lengths` - A vector of the lengths of each word in the document.
/// * `max_width` - The maximum line width.
///
/// # Returns
///
/// A vector of indices that represent the end of each line in the justified text.
/// Score struct to hold the "badness" and the index of the next word
#[derive(Clone, Copy, Debug)]
pub struct Score {
    pub badness: usize,
    pub j: usize,
}

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
