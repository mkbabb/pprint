/**
 * Greedy text justification: packs items into lines, breaking when the next
 * item would exceed `maxWidth`. Returns break indices — each index is the
 * position where a new line starts.
 *
 * Port of Rust `text_justify()` from `pprint/rust/src/utils.rs`.
 */
export function textJustify(
    sepLength: number,
    docLengths: number[],
    maxWidth: number,
): number[] {
    const breaks: number[] = [];
    const n = docLengths.length;
    let i = 0;
    while (i < n) {
        let lineLength = docLengths[i]!;
        let j = i + 1;
        while (j < n) {
            const next = lineLength + sepLength + docLengths[j]!;
            if (next > maxWidth) {
                break;
            }
            lineLength = next;
            j += 1;
        }
        breaks.push(j);
        i = j;
    }
    return breaks;
}
