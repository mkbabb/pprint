export type Doc =
    | { type: "null" }
    | { type: "text"; value: string }
    | { type: "concat"; docs: Doc[] }
    | { type: "group"; doc: Doc }
    | { type: "indent"; doc: Doc }
    | { type: "dedent"; doc: Doc }
    | { type: "join"; sep: Doc; docs: Doc[] }
    | { type: "smartJoin"; sep: Doc; docs: Doc[] }
    | { type: "linearJoin"; sep: Doc; docs: Doc[] }
    | { type: "ifBreak"; broken: Doc; flat: Doc }
    | { type: "hardline" }
    | { type: "softline" }
    | { type: "mediumline" }
    | { type: "line" };

// Singleton constants
export const NULL_DOC: Doc = { type: "null" };
export const hardline: Doc = { type: "hardline" };
export const softline: Doc = { type: "softline" };
export const mediumline: Doc = { type: "mediumline" };
export const line: Doc = { type: "line" };

export function text(value: string): Doc {
    return { type: "text", value };
}

export function group(doc: Doc): Doc {
    return { type: "group", doc };
}

export function indent(doc: Doc): Doc {
    return { type: "indent", doc };
}

export function dedent(doc: Doc): Doc {
    return { type: "dedent", doc };
}

export function concat(...docs: Doc[]): Doc {
    const filtered: Doc[] = [];
    for (const d of docs) {
        if (d.type === "null") continue;
        if (d.type === "concat") {
            filtered.push(...d.docs);
        } else {
            filtered.push(d);
        }
    }
    if (filtered.length === 0) return NULL_DOC;
    if (filtered.length === 1) return filtered[0]!;
    return { type: "concat", docs: filtered };
}

export function join(sep: Doc, docs: Doc[]): Doc {
    return { type: "join", sep, docs };
}

export function smartJoin(sep: Doc, docs: Doc[]): Doc {
    return { type: "smartJoin", sep, docs };
}

export function linearJoin(sep: Doc, docs: Doc[]): Doc {
    return { type: "linearJoin", sep, docs };
}

export function ifBreak(broken: Doc, flat: Doc): Doc {
    return { type: "ifBreak", broken, flat };
}

export function wrap(left: Doc, doc: Doc, right: Doc): Doc {
    return concat(left, doc, right);
}
