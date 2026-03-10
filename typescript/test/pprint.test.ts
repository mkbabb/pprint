import { describe, it, expect } from "vitest";
import {
    pprint,
    text,
    concat,
    group,
    indent,
    dedent,
    join,
    smartJoin,
    linearJoin,
    ifBreak,
    hardline,
    softline,
    mediumline,
    line,
    wrap,
    NULL_DOC,
    textJustify,
} from "../src/index.js";

describe("text", () => {
    it("renders plain text", () => {
        expect(pprint(text("hello"))).toBe("hello");
    });

    it("renders empty text", () => {
        expect(pprint(text(""))).toBe("");
    });
});

describe("NULL_DOC", () => {
    it("renders nothing", () => {
        expect(pprint(NULL_DOC)).toBe("");
    });
});

describe("concat", () => {
    it("joins multiple texts", () => {
        expect(pprint(concat(text("a"), text("b"), text("c")))).toBe("abc");
    });

    it("filters null docs", () => {
        expect(pprint(concat(text("a"), NULL_DOC, text("b")))).toBe("ab");
    });

    it("flattens nested concats", () => {
        const inner = concat(text("a"), text("b"));
        const outer = concat(inner, text("c"));
        expect(pprint(outer)).toBe("abc");
    });

    it("returns NULL_DOC for all nulls", () => {
        const result = concat(NULL_DOC, NULL_DOC);
        expect(result.type).toBe("null");
    });

    it("returns single doc when only one remains", () => {
        const result = concat(NULL_DOC, text("x"));
        expect(result).toEqual(text("x"));
    });
});

describe("hardline", () => {
    it("always emits a line break with indent", () => {
        const doc = concat(text("a"), hardline, text("b"));
        expect(pprint(doc)).toBe("a\nb");
    });

    it("respects indent level", () => {
        const doc = indent(concat(text("a"), hardline, text("b")));
        expect(pprint(doc)).toBe("a\n    b");
    });
});

describe("line", () => {
    it("emits bare newline without indent", () => {
        const doc = concat(text("a"), line, text("b"));
        expect(pprint(doc)).toBe("a\nb");
    });

    it("does not add indent even inside indent()", () => {
        // Line in Rust always outputs just '\n' with currentLineLen = 0
        const doc = indent(concat(text("a"), line, text("b")));
        expect(pprint(doc)).toBe("a\nb");
    });
});

describe("softline", () => {
    it("does not emit space when within width", () => {
        const doc = concat(text("a"), softline, text("b"));
        expect(pprint(doc)).toBe("ab");
    });

    it("breaks when current line exceeds maxWidth", () => {
        // Create a line that's already at maxWidth
        const long = text("x".repeat(81));
        const doc = concat(long, softline, text("y"));
        expect(pprint(doc, { maxWidth: 80 })).toBe("x".repeat(81) + "\ny");
    });
});

describe("mediumline", () => {
    it("does not emit space when within half width", () => {
        const doc = concat(text("a"), mediumline, text("b"));
        expect(pprint(doc, { maxWidth: 80 })).toBe("ab");
    });

    it("breaks when current line exceeds half maxWidth", () => {
        const long = text("x".repeat(41));
        const doc = concat(long, mediumline, text("y"));
        expect(pprint(doc, { maxWidth: 80 })).toBe("x".repeat(41) + "\ny");
    });
});

describe("group", () => {
    it("keeps content flat when it fits", () => {
        const doc = group(
            concat(text("["), ifBreak(hardline, NULL_DOC), text("a, b"), ifBreak(hardline, NULL_DOC), text("]")),
        );
        expect(pprint(doc, { maxWidth: 80 })).toBe("[a, b]");
    });

    it("breaks when content does not fit", () => {
        const items = text("a".repeat(40) + ", " + "b".repeat(40));
        const doc = group(
            concat(
                text("["),
                ifBreak(concat(hardline, text("  ")), NULL_DOC),
                items,
                ifBreak(hardline, NULL_DOC),
                text("]"),
            ),
        );
        const result = pprint(doc, { maxWidth: 40 });
        expect(result).toContain("\n");
    });

    it("measures width including currentLineLen", () => {
        // Put some text before the group so currentLineLen is non-zero
        const prefix = text("prefix: "); // 8 chars
        const inner = text("x".repeat(73)); // 73 chars, total 81 > 80
        const doc = concat(
            prefix,
            group(concat(ifBreak(hardline, NULL_DOC), inner)),
        );
        const result = pprint(doc, { maxWidth: 80 });
        // Group should break because 8 + 73 = 81 > 80
        expect(result).toBe("prefix: \n" + "x".repeat(73));
    });
});

describe("indent / dedent", () => {
    it("increases indent for nested content", () => {
        const doc = indent(concat(hardline, text("indented")));
        expect(pprint(doc, { indent: 4 })).toBe("\n    indented");
    });

    it("stacks indent levels", () => {
        const doc = indent(
            indent(concat(hardline, text("double"))),
        );
        expect(pprint(doc, { indent: 4 })).toBe("\n        double");
    });

    it("dedent reduces indent", () => {
        const doc = indent(
            concat(
                hardline,
                text("in"),
                dedent(concat(hardline, text("out"))),
            ),
        );
        expect(pprint(doc, { indent: 4 })).toBe("\n    in\nout");
    });

    it("dedent does not go below zero", () => {
        const doc = dedent(concat(hardline, text("floor")));
        expect(pprint(doc, { indent: 4 })).toBe("\nfloor");
    });

    it("uses tabs when configured", () => {
        const doc = indent(concat(hardline, text("tabbed")));
        expect(pprint(doc, { indent: 4, useTabs: true })).toBe(
            "\n" + "\t".repeat(4) + "tabbed",
        );
    });
});

describe("ifBreak", () => {
    it("uses flat branch when not in break mode", () => {
        const doc = ifBreak(text("BROKEN"), text("flat"));
        expect(pprint(doc)).toBe("flat");
    });

    it("uses broken branch when group breaks", () => {
        const inner = ifBreak(text("\nBROKEN"), text("flat"));
        const doc = group(concat(text("x".repeat(80)), inner));
        const result = pprint(doc, { maxWidth: 80 });
        expect(result).toContain("BROKEN");
    });
});

describe("join", () => {
    it("joins docs with separator", () => {
        const doc = join(text(", "), [text("a"), text("b"), text("c")]);
        expect(pprint(doc)).toBe("a, b, c");
    });

    it("handles empty docs", () => {
        const doc = join(text(", "), []);
        expect(pprint(doc)).toBe("");
    });

    it("handles single doc", () => {
        const doc = join(text(", "), [text("only")]);
        expect(pprint(doc)).toBe("only");
    });

    it("propagates break mode from parent group", () => {
        const items = [text("aaa"), text("bbb"), text("ccc")];
        const sep = ifBreak(concat(text(","), hardline), text(", "));
        const doc = group(join(sep, items));
        // Fits on one line
        expect(pprint(doc, { maxWidth: 80 })).toBe("aaa, bbb, ccc");
    });
});

describe("smartJoin", () => {
    it("keeps everything on one line when it fits", () => {
        const docs = [text("a"), text("b"), text("c")];
        const doc = smartJoin(text(" "), docs);
        expect(pprint(doc, { maxWidth: 80 })).toBe("a b c");
    });

    it("breaks into multiple lines via text_justify", () => {
        const docs = [
            text("aaaa"),
            text("bbbb"),
            text("cccc"),
            text("dddd"),
            text("eeee"),
        ];
        const doc = smartJoin(text(" "), docs);
        const result = pprint(doc, { maxWidth: 15 });
        // Each word is 4 chars, sep is 1. Line can fit: 4+1+4+1+4 = 14 <= 15
        // So 3 per line: "aaaa bbbb cccc" (14), then "dddd eeee" (9)
        expect(result).toBe("aaaa bbbb cccc\ndddd eeee");
    });
});

describe("linearJoin", () => {
    it("keeps everything on one line when it fits", () => {
        const docs = [text("a"), text("b"), text("c")];
        const doc = linearJoin(text(" "), docs);
        expect(pprint(doc, { maxWidth: 80 })).toBe("a b c");
    });

    it("breaks when line would overflow", () => {
        const docs = [
            text("aaaa"),
            text("bbbb"),
            text("cccc"),
            text("dddd"),
        ];
        const doc = linearJoin(text(" "), docs);
        const result = pprint(doc, { maxWidth: 15 });
        // 4+1+4+1+4 = 14 fits, then +1+4 = 19 > 15, so break
        expect(result).toBe("aaaa bbbb cccc\ndddd");
    });

    it("accounts for currentLineLen from preceding text", () => {
        const docs = [text("aa"), text("bb"), text("cc")];
        const doc = concat(text("prefix: "), linearJoin(text(" "), docs));
        // prefix is 8 chars. 8+2=10, 10+1+2=13, 13+1+2=16 > 15
        const result = pprint(doc, { maxWidth: 15 });
        expect(result).toBe("prefix: aa bb\ncc");
    });
});

describe("textJustify", () => {
    it("returns empty for empty input", () => {
        expect(textJustify(1, [], 80)).toEqual([]);
    });

    it("packs items greedily", () => {
        // Items: 4, 4, 4, 4, 4. Sep: 1. MaxWidth: 14.
        // Line 1: 4+1+4+1+4 = 14 (fits). Next break at index 3.
        // Line 2: 4+1+4 = 9 (fits). Next break at index 5.
        const result = textJustify(1, [4, 4, 4, 4, 4], 14);
        expect(result).toEqual([3, 5]);
    });

    it("single item per line when each exceeds width", () => {
        const result = textJustify(1, [10, 10, 10], 9);
        // Each item alone exceeds width, but first always placed.
        // Line 1: 10, break at 1. Line 2: 10, break at 2. Line 3: 10, break at 3.
        expect(result).toEqual([1, 2, 3]);
    });
});

describe("wrap", () => {
    it("wraps doc with left and right", () => {
        const doc = wrap(text("["), text("inner"), text("]"));
        expect(pprint(doc)).toBe("[inner]");
    });
});

describe("nested groups", () => {
    it("outer breaks, inner stays flat", () => {
        const inner = group(
            concat(text("("), ifBreak(hardline, NULL_DOC), text("short"), ifBreak(hardline, NULL_DOC), text(")")),
        );
        const outer = group(
            concat(
                text("x".repeat(75)),
                ifBreak(concat(text(","), hardline), text(", ")),
                inner,
            ),
        );
        const result = pprint(outer, { maxWidth: 80 });
        // Outer should break: flat width = 75 + 2 + 7 = 84 > 80
        // Inner (short) should stay flat since its own width is small
        expect(result).toContain(",\n");
        expect(result).toContain("(short)");
    });
});

describe("real-world: JSON-like formatting", () => {
    it("formats array inline when short", () => {
        const items = [text("1"), text("2"), text("3")];
        const sep = ifBreak(concat(text(","), hardline), text(", "));
        const doc = group(
            concat(
                text("["),
                indent(concat(ifBreak(hardline, NULL_DOC), join(sep, items))),
                ifBreak(hardline, NULL_DOC),
                text("]"),
            ),
        );
        expect(pprint(doc, { maxWidth: 80 })).toBe("[1, 2, 3]");
    });

    it("formats array with breaks when long", () => {
        const items = [
            text('"' + "a".repeat(30) + '"'),
            text('"' + "b".repeat(30) + '"'),
            text('"' + "c".repeat(30) + '"'),
        ];
        const sep = ifBreak(concat(text(","), hardline), text(", "));
        const doc = group(
            concat(
                text("["),
                indent(concat(ifBreak(hardline, NULL_DOC), join(sep, items))),
                ifBreak(hardline, NULL_DOC),
                text("]"),
            ),
        );
        const result = pprint(doc, { maxWidth: 80, indent: 4 });
        const lines = result.split("\n");
        expect(lines.length).toBe(5); // [, 3 items, ]
        expect(lines[0]).toBe("[");
        expect(lines[1]).toBe('    "' + "a".repeat(30) + '",');
        expect(lines[2]).toBe('    "' + "b".repeat(30) + '",');
        expect(lines[3]).toBe('    "' + "c".repeat(30) + '"');
        expect(lines[4]).toBe("]");
    });
});

describe("indent size configuration", () => {
    it("respects indent: 2", () => {
        const doc = indent(concat(hardline, text("hi")));
        expect(pprint(doc, { indent: 2 })).toBe("\n  hi");
    });

    it("respects indent: 8", () => {
        const doc = indent(concat(hardline, text("hi")));
        expect(pprint(doc, { indent: 8 })).toBe("\n        hi");
    });
});

describe("smartJoin with non-literal separator", () => {
    it("handles ifBreak separator in smartJoin", () => {
        const sep = ifBreak(concat(text(","), hardline), text(", "));
        const docs = [text("aaa"), text("bbb"), text("ccc")];
        // Non-literal sep gets pushed as separate stack items
        const doc = smartJoin(sep, docs);
        expect(pprint(doc, { maxWidth: 80 })).toBe("aaa, bbb, ccc");
    });
});

describe("strip trailing whitespace on break", () => {
    it("strips spaces before smartJoin line break", () => {
        // When smartJoin breaks, trailing whitespace before the break should be stripped.
        const docs = [
            text("aaaa"),
            text("bbbb"),
            text("cccc"),
        ];
        // Use a separator with trailing space
        const doc = smartJoin(text(", "), docs);
        const result = pprint(doc, { maxWidth: 12 });
        // Should not have trailing spaces before newlines
        for (const ln of result.split("\n")) {
            expect(ln).toBe(ln.trimEnd());
        }
    });
});
