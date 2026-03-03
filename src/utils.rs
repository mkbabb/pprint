
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

/// Greedy line breaking: pack as many items as fit on each line.
/// O(n) — used as fallback when n is large to avoid O(n^2) DP.
#[inline]
fn text_justify_greedy(
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

/// Threshold above which we switch from O(n^2) DP to O(n) greedy.
const GREEDY_THRESHOLD: usize = 32;

#[inline]
pub fn text_justify(
    sep_length: usize,
    doc_lengths: &[usize],
    max_width: usize,
    output: &mut Vec<usize>,
    memo: &mut Vec<Score>,
) {
    let n = doc_lengths.len();

    // For large item counts, use greedy O(n) instead of O(n^2) DP.
    if n > GREEDY_THRESHOLD {
        return text_justify_greedy(sep_length, doc_lengths, max_width, output);
    }

    // Initialize memoization vector with maximum badness and the index of the next word
    memo.clear();
    memo.resize(
        n + 1,
        Score {
            badness: usize::MAX,
            j: n,
        },
    );
    // The last word has no badness and does not point to any next word
    memo[n] = Score { badness: 0, j: 0 };

    // Iterate over the words in reverse order
    for i in (0..=n).rev() {
        let mut line_length = 0;

        // For each word, calculate the line length and badness
        for j in i..n {
            // Add the length of the current word to the line length
            line_length += doc_lengths[j];
            // Add the separator length if this is not the first word in the line
            if j > i {
                line_length += sep_length;
            }
            // Calculate badness: overflow is heavily penalized, underflow uses cube.
            let badness = if line_length > max_width {
                // Overflow penalty — much worse than any fitting arrangement.
                // Still compute so that fewer-overflow options win over more-overflow.
                let overflow = line_length - max_width;
                usize::MAX / 2 + overflow.saturating_pow(3)
            } else {
                (max_width - line_length).saturating_pow(3)
            };
            let next_score = memo[j + 1];

            let total_badness = badness.saturating_add(next_score.badness);
            if total_badness <= memo[i].badness {
                memo[i] = Score {
                    badness: total_badness,
                    j: j + 1,
                };
            }
            // Once past max_width, adding more items only makes it worse.
            if line_length > max_width {
                break;
            }
        }
    }

    // Generate the list of line breaks by scanning the memoization vector
    let mut i = 0;
    while i < n {
        let j = memo[i].j;
        output.push(j);
        i = j;
    }
}
