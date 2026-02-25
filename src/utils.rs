
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
#[inline]
pub fn text_justify(
    sep_length: usize,
    doc_lengths: &[usize],
    max_width: usize,
    output: &mut Vec<usize>,
) {
    // Score struct to hold the "badness" and the index of the next word
    #[derive(Clone, Debug)]
    struct Score {
        badness: usize,
        j: usize,
    }

    // Initialize memoization vector with maximum badness and the index of the next word
    let n = doc_lengths.len();
    let mut memo = vec![
        Score {
            badness: usize::MAX,
            j: n
        };
        n + 1
    ];
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
            // Ensure that the line length does not exceed the maximum width
            line_length = line_length.min(max_width);

            // Calculate the badness as the cube of the unused space at the end of the line
            let badness = (max_width - line_length).pow(3);
            // Get the score of the next word
            let next_score = memo[j + 1].clone();

            // If the total badness of this line and the next is less than the current badness,
            // update the score for this word
            if badness + next_score.badness < memo[i].badness {
                memo[i] = Score {
                    badness: badness + next_score.badness,
                    j: j + 1,
                };
            }
            // If the line length has reached the maximum width, break the loop
            if line_length >= max_width {
                break;
            }
        }
    }

    // Generate the list of line breaks by scanning the memoization vector
    // (0..n)
    //     .scan(0, |i, _| {
    //         let j = memo[*i].j;
    //         *i = j;
    //         Some(j)
    //     })
    //     .collect_into(&mut output)

    let mut i = 0;
    while i < n {
        let j = memo[i].j;
        output.push(j);
        i = j;
    }

   
}
