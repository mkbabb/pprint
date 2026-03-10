import type { Doc } from "./doc.js";
import { textJustify } from "./utils.js";

export interface PrinterConfig {
    maxWidth?: number; // default 80
    indent?: number; // default 4 (matches Rust PRINTER default)
    useTabs?: boolean; // default false
}

interface StackItem {
    doc: Doc;
    indentDelta: number;
    breakMode: boolean;
    /** Literal separator to emit before this item (Join optimization). */
    left: Doc | null;
    /** If >= 0, emit a line break (with this indent) before this item. -1 means no break. */
    breakLeft: number;
}

/**
 * Check if a Doc is "literal" — can be rendered immediately without stack processing.
 * Matches Rust `is_literal_doc`.
 */
function isLiteralDoc(doc: Doc): boolean {
    switch (doc.type) {
        case "text":
        case "hardline":
        case "softline":
        case "mediumline":
        case "line":
            return true;
        default:
            return false;
    }
}

/**
 * Count the text width of a Doc (flat / inline measurement).
 * Matches Rust `count_text_length`.
 */
function countTextLength(
    doc: Doc,
    maxWidth: number,
    cache: Map<Doc, number>,
): number {
    const cached = cache.get(doc);
    if (cached !== undefined) return cached;

    let w: number;
    switch (doc.type) {
        case "null":
            w = 0;
            break;
        case "text":
            w = doc.value.length;
            break;
        // Softline measures as 1 (potential inline contribution).
        case "softline":
            w = 1;
            break;
        // Mediumline measures as 0 in Rust.
        case "mediumline":
            w = 0;
            break;
        // Hardline and Line force a break — measure as maxWidth to force Group breaking.
        case "hardline":
        case "line":
            w = maxWidth;
            break;
        case "concat":
            w = 0;
            for (const d of doc.docs) {
                w += countTextLength(d, maxWidth, cache);
            }
            break;
        case "group":
            w = countTextLength(doc.doc, maxWidth, cache);
            break;
        case "indent":
            w = countTextLength(doc.doc, maxWidth, cache);
            break;
        case "dedent":
            w = countTextLength(doc.doc, maxWidth, cache);
            break;
        case "ifBreak":
            // Measure the flat branch (break_mode=false).
            w = countTextLength(doc.flat, maxWidth, cache);
            break;
        case "join":
        case "smartJoin":
        case "linearJoin": {
            if (doc.docs.length === 0) {
                w = 0;
            } else {
                const sepW = countTextLength(doc.sep, maxWidth, cache);
                w = 0;
                for (let i = 0; i < doc.docs.length; i++) {
                    w += countTextLength(doc.docs[i]!, maxWidth, cache);
                    if (i > 0) w += sepW;
                }
            }
            break;
        }
    }

    cache.set(doc, w);
    return w;
}

/**
 * Append a newline + indentation to the output buffer.
 * Returns the new `currentLineLen` (= indentDelta).
 */
function appendLine(
    output: string[],
    indentDelta: number,
    useTabs: boolean,
): number {
    const indentStr = useTabs
        ? "\t".repeat(indentDelta)
        : " ".repeat(indentDelta);
    output.push("\n" + indentStr);
    return indentDelta;
}

/**
 * Handle a literal doc — emit its bytes and return the new currentLineLen.
 */
function handleLiteral(
    doc: Doc,
    output: string[],
    currentLineLen: number,
    indentDelta: number,
    maxWidth: number,
    useTabs: boolean,
): number {
    switch (doc.type) {
        case "null":
            return currentLineLen;
        case "text":
            output.push(doc.value);
            return currentLineLen + doc.value.length;
        case "line":
            output.push("\n");
            return 0;
        case "hardline":
            return appendLine(output, indentDelta, useTabs);
        case "mediumline":
            if (currentLineLen > maxWidth / 2) {
                return appendLine(output, indentDelta, useTabs);
            }
            return currentLineLen;
        case "softline":
            if (currentLineLen > maxWidth) {
                return appendLine(output, indentDelta, useTabs);
            }
            return currentLineLen;
        default:
            throw new Error(`handleLiteral: unexpected doc type "${doc.type}"`);
    }
}

/**
 * Strip trailing whitespace from the output buffer (spaces and tabs).
 */
function stripTrailingWhitespace(output: string[]): void {
    while (output.length > 0) {
        const last = output[output.length - 1]!;
        if (last === " " || last === "\t") {
            output.pop();
            continue;
        }
        const trimmed = last.replace(/[ \t]+$/, "");
        if (trimmed.length < last.length) {
            if (trimmed.length === 0) {
                output.pop();
            } else {
                output[output.length - 1] = trimmed;
            }
            return;
        }
        return;
    }
}

/**
 * Core pretty-printing function.
 *
 * Stack-based renderer matching Rust `pprint()` from `pprint/rust/src/print.rs`.
 */
export function pprint(doc: Doc, config: PrinterConfig = {}): string {
    const maxWidth = config.maxWidth ?? 80;
    const indentSize = config.indent ?? 4;
    const useTabs = config.useTabs ?? false;

    const output: string[] = [];
    let currentLineLen = 0;
    let indentDelta = 0;

    const widthCache = new Map<Doc, number>();

    const stack: StackItem[] = [
        {
            doc,
            indentDelta: 0,
            breakMode: false,
            left: null,
            breakLeft: -1,
        },
    ];

    while (stack.length > 0) {
        const item = stack.pop()!;

        // Emit literal separator before this item (Join optimization).
        if (item.left !== null) {
            currentLineLen = handleLiteral(
                item.left,
                output,
                currentLineLen,
                item.indentDelta,
                maxWidth,
                useTabs,
            );
        }

        // Emit line break before this item (SmartJoin/LinearJoin break position).
        // breakLeft >= 0 means "break with this indent level".
        if (item.breakLeft >= 0) {
            stripTrailingWhitespace(output);
            currentLineLen = appendLine(output, item.breakLeft, useTabs);
        }

        // Resolve Indent/Dedent — unwrap (possibly multiple layers) and adjust
        // indentDelta before dispatch.
        let current = item.doc;
        let itemIndent = item.indentDelta;
        const breakMode = item.breakMode;

        while (current.type === "indent" || current.type === "dedent") {
            if (current.type === "indent") {
                itemIndent = Math.min(itemIndent + indentSize, maxWidth);
                current = current.doc;
            } else {
                itemIndent = Math.max(0, itemIndent - indentSize);
                current = current.doc;
            }
        }

        indentDelta = itemIndent;

        switch (current.type) {
            case "null":
                break;

            case "text":
                output.push(current.value);
                currentLineLen += current.value.length;
                break;

            case "line":
                output.push("\n");
                currentLineLen = 0;
                break;

            case "hardline":
                currentLineLen = appendLine(output, indentDelta, useTabs);
                break;

            case "softline":
                if (currentLineLen > maxWidth) {
                    currentLineLen = appendLine(output, indentDelta, useTabs);
                }
                break;

            case "mediumline":
                if (currentLineLen > maxWidth / 2) {
                    currentLineLen = appendLine(output, indentDelta, useTabs);
                }
                break;

            case "concat":
                for (let i = current.docs.length - 1; i >= 0; i--) {
                    stack.push({
                        doc: current.docs[i]!,
                        indentDelta: itemIndent,
                        breakMode,
                        left: null,
                        breakLeft: -1,
                    });
                }
                break;

            case "group": {
                const groupWidth = countTextLength(
                    current.doc,
                    maxWidth,
                    widthCache,
                );
                const needsBreaking =
                    currentLineLen + groupWidth > maxWidth;
                stack.push({
                    doc: current.doc,
                    indentDelta: itemIndent,
                    breakMode: needsBreaking,
                    left: null,
                    breakLeft: -1,
                });
                break;
            }

            case "ifBreak":
                stack.push({
                    doc: breakMode ? current.broken : current.flat,
                    indentDelta: itemIndent,
                    breakMode,
                    left: null,
                    breakLeft: -1,
                });
                break;

            case "join":
            case "smartJoin": {
                const sep = current.sep;
                const docs = current.docs;
                const isSmartJoin = current.type === "smartJoin";

                // Compute break positions for SmartJoin.
                // textJustify returns indices where each new line STARTS.
                // The last entry is past-end (docs.length) and should be ignored.
                let breakSet: Set<number> | null = null;
                if (isSmartJoin && docs.length > 0) {
                    const adjustedMax = Math.max(
                        0,
                        maxWidth - indentDelta,
                    );
                    const sepLen = countTextLength(
                        sep,
                        maxWidth,
                        widthCache,
                    );
                    const docLengths = docs.map((d) =>
                        countTextLength(d, maxWidth, widthCache),
                    );
                    const breakIndices = textJustify(
                        sepLen,
                        docLengths,
                        adjustedMax,
                    );
                    // Each index in breakIndices is where the NEXT line starts.
                    // Filter out the past-end sentinel.
                    breakSet = new Set<number>();
                    for (const idx of breakIndices) {
                        if (idx < docs.length) {
                            breakSet.add(idx);
                        }
                    }
                }

                const sepIsLit = isLiteralDoc(sep);

                for (let i = docs.length - 1; i >= 0; i--) {
                    const needsBreak =
                        breakSet !== null && breakSet.has(i);
                    stack.push({
                        doc: docs[i]!,
                        indentDelta: itemIndent,
                        breakMode,
                        left:
                            i > 0 && sepIsLit && !needsBreak
                                ? sep
                                : null,
                        breakLeft: needsBreak ? indentDelta : -1,
                    });
                    if (!sepIsLit && i > 0 && !needsBreak) {
                        stack.push({
                            doc: sep,
                            indentDelta: itemIndent,
                            breakMode,
                            left: null,
                            breakLeft: -1,
                        });
                    }
                }
                break;
            }

            case "linearJoin": {
                const sep = current.sep;
                const docs = current.docs;

                if (docs.length === 0) break;

                const adjustedMax = Math.max(0, maxWidth - indentDelta);
                const sepLen = countTextLength(sep, maxWidth, widthCache);
                const sepIsLit = isLiteralDoc(sep);

                // Forward scan: compute break positions using currentLineLen.
                const breakSet = new Set<number>();
                let lineLen = currentLineLen;

                for (let i = 0; i < docs.length; i++) {
                    const itemWidth = countTextLength(
                        docs[i]!,
                        maxWidth,
                        widthCache,
                    );
                    if (i > 0) {
                        const nextLen = lineLen + sepLen + itemWidth;
                        if (nextLen > adjustedMax) {
                            breakSet.add(i);
                            lineLen = indentDelta + itemWidth;
                        } else {
                            lineLen = nextLen;
                        }
                    } else {
                        lineLen += itemWidth;
                    }
                }

                // Push in reverse.
                for (let i = docs.length - 1; i >= 0; i--) {
                    const needsBreak = breakSet.has(i);
                    stack.push({
                        doc: docs[i]!,
                        indentDelta: itemIndent,
                        breakMode: false,
                        left:
                            i > 0 && sepIsLit && !needsBreak
                                ? sep
                                : null,
                        breakLeft: needsBreak ? indentDelta : -1,
                    });
                    if (!sepIsLit && i > 0 && !needsBreak) {
                        stack.push({
                            doc: sep,
                            indentDelta: itemIndent,
                            breakMode: false,
                            left: null,
                            breakLeft: -1,
                        });
                    }
                }
                break;
            }
        }
    }

    return output.join("");
}
