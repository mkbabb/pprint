/// Flat formatting operations — the instruction set for the linear renderer.
///
/// Each op is a small, copyable value. The entire formatting program is a
/// `Vec<FmtOp>` — no tree structure, no Box allocations, no pointer chasing.
/// Sequential ops are implicitly concatenated.
///
/// Size: 24 bytes (8-byte alignment). The largest inline variant is `Text(&str)` at 16 bytes.
#[derive(Debug, Clone)]
pub enum FmtOp<'a> {
    /// Emit borrowed source text. Zero-copy from the parsed input.
    Text(&'a str),

    /// Emit computed/formatted text. Owned string for text_owned() path
    /// (hex formatting, Display fallbacks). Rare — the 99% path is Text.
    OwnedText(String),

    /// Emit borrowed text wrapped in double quotes. Fused fast-path for
    /// `Char('"') + Text(s) + Char('"')` — single op, single match dispatch.
    QuotedText(&'a str),

    /// Emit a single ASCII byte (delimiters, operators, commas).
    Char(u8),

    // ── Numeric ops (rendered inline by the renderer) ───────────────
    /// Signed 64-bit integer. Covers i8, i16, i32, isize via widening.
    I64(i64),

    /// Unsigned 64-bit integer. Covers u8, u16, u32, usize via widening.
    U64(u64),

    /// High 64 bits of a signed 128-bit integer (wide-op: next op must be `I128Lo`).
    /// The renderer reconstructs `i128` from `(hi as i128) << 64 | lo as i128`.
    I128Hi(u64),
    /// Low 64 bits of a signed 128-bit integer (must follow `I128Hi`).
    I128Lo(u64),

    /// High 64 bits of an unsigned 128-bit integer (wide-op: next op must be `U128Lo`).
    U128Hi(u64),
    /// Low 64 bits of an unsigned 128-bit integer (must follow `U128Hi`).
    U128Lo(u64),

    /// 64-bit float. Covers f32 via cast. Rendered with Dragonbox.
    F64(f64),

    // ── Structural ops ──────────────────────────────────────────────
    /// Begin a group region. The renderer checks:
    ///   `current_line_len + flat_width > max_width` → set break mode for children.
    /// `flat_width` is pre-computed during construction by the `FmtBuilder`.
    GroupStart { flat_width: u16 },

    /// End the innermost group region.
    GroupEnd,

    /// Increase indentation for subsequent lines.
    IndentStart,

    /// Decrease indentation for subsequent lines.
    IndentEnd,

    /// Begin a wrapped container: group + open delimiter + indent + break_line.
    /// Combines GroupStart + Char(open) + IndentStart + BreakLine into one op.
    WrapStart { open: u8, flat_width: u16 },

    /// End a wrapped container: indent_end + break_line + close char + group_end.
    /// Combines IndentEnd + BreakLine + Char(close) + GroupEnd into one op.
    WrapEnd { close: u8 },

    /// Unconditional line break. Always emits newline + indent.
    Hardline,

    /// In break mode: newline + indent. In flat mode: single space.
    Softline,

    /// In break mode: newline + indent. In flat mode: nothing.
    BreakLine,

    /// Line break when `current_line_len > max_width / 2`, otherwise nothing.
    Mediumline,

    /// The universal comma separator: flat = `", "`, break = `","` + newline.
    /// Zero-payload fast path for the most common separator pattern.
    CommaSep,

    /// Separator: in break mode emit `brk` bytes + newline + indent;
    /// in flat mode emit `flat` bytes. For non-comma separators.
    Sep {
        flat: [u8; 8],
        flat_len: u8,
        brk: [u8; 4],
        brk_len: u8,
    },

    /// In break mode: emit next `break_count` ops, then skip `flat_count`.
    /// In flat mode: skip `break_count` ops, then emit `flat_count`.
    IfBreak { break_count: u16, flat_count: u16 },
}
