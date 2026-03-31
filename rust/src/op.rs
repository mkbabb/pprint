/// Flat formatting operations — the instruction set for the linear renderer.
///
/// Each op is a small, copyable value. The entire formatting program is a
/// `Vec<FmtOp>` — no tree structure, no Box allocations, no pointer chasing.
/// Sequential ops are implicitly concatenated.
#[derive(Debug, Clone, Copy)]
pub enum FmtOp<'a> {
    /// Emit borrowed source text. Zero-copy from the parsed input.
    Text(&'a str),

    /// Emit a single ASCII byte (delimiters, operators, commas).
    Char(u8),

    // ── Numeric ops (rendered inline by the renderer) ───────────────

    /// Signed 64-bit integer. Covers i8, i16, i32, isize via widening.
    I64(i64),

    /// Unsigned 64-bit integer. Covers u8, u16, u32, usize via widening.
    U64(u64),

    /// Signed 128-bit integer.
    I128(i128),

    /// Unsigned 128-bit integer.
    U128(u128),

    /// 64-bit float. Covers f32 via cast. Rendered with Dragonbox.
    F64(f64),

    // ── Structural ops ──────────────────────────────────────────────

    /// Begin a group region. The renderer checks:
    ///   `current_line_len + flat_width > max_width` → set break mode for children.
    /// `flat_width` is pre-computed during construction by the `FmtBuilder`.
    GroupStart { flat_width: u32 },

    /// End the innermost group region.
    GroupEnd,

    /// Increase indentation for subsequent lines.
    IndentStart,

    /// Decrease indentation for subsequent lines.
    IndentEnd,

    /// Unconditional line break. Always emits newline + indent.
    Hardline,

    /// In break mode: newline + indent. In flat mode: single space.
    Softline,

    /// In break mode: newline + indent. In flat mode: nothing.
    BreakLine,

    /// Line break when `current_line_len > max_width / 2`, otherwise nothing.
    /// Useful for code formatting where earlier breaks are preferred.
    Mediumline,

    /// In break mode: emit next `break_count` ops, then skip `flat_count`.
    /// In flat mode: skip `break_count` ops, then emit `flat_count`.
    IfBreak {
        break_count: u16,
        flat_count: u16,
    },
}
