/// Flat formatting operations — the instruction set for the linear renderer.
///
/// Each op is a small, copyable value. The entire formatting program is a
/// `Vec<FmtOp>` — no tree structure, no Box allocations, no pointer chasing.
/// Sequential ops are implicitly concatenated.
///
/// ABI v2 (sk-v27 LW2.PP-1). Size: 24 bytes, 8-byte alignment, const-asserted
/// below. `OwnedText(String)` is the widest payload; the enum fits in 24 bytes
/// only through the niche in `String`'s capacity field, so the assert is the
/// guard against losing it. Group metadata (flat width, trailing width, end,
/// flags) lives in the [`GroupHdr`] side table, indexed by `hdr`, with u32
/// fields. The layout record is `rust/ABI-v2.md`.
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
    /// Begin a group region: [W] §3 `group x = flatten x :<|> x`.
    /// `hdr` indexes the document's [`GroupHdr`] table. The renderer chooses
    /// flat if and only if the group is not `HAS_HARD` and
    /// `column + flat_width + trailing <= max_width` (laws L1, L2, L3′).
    ///
    /// While a [`FmtBuilder`](crate::FmtBuilder) is still open, `hdr` holds
    /// the group's running flat width; `finish()` replaces it with the index.
    GroupStart { hdr: u32 },

    /// End the innermost group region.
    GroupEnd,

    /// Increase indentation for subsequent lines.
    IndentStart,

    /// Decrease indentation for subsequent lines.
    IndentEnd,

    /// Begin a wrapped container: group + open delimiter + indent + break_line.
    /// Combines GroupStart + Char(open) + IndentStart + BreakLine into one op.
    /// `hdr` is as for `GroupStart`; the flat width counts both delimiters.
    WrapStart { open: u8, hdr: u32 },

    /// End a wrapped container: indent_end + break_line + close char + group_end.
    /// Combines IndentEnd + BreakLine + Char(close) + GroupEnd into one op.
    WrapEnd { close: u8 },

    /// Unconditional line break. Always emits newline + indent.
    Hardline,

    /// In break mode: newline + indent. In flat mode: single space.
    Softline,

    /// In break mode: newline + indent. In flat mode: nothing.
    BreakLine,

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

    /// [P] `flatAlt`: in break mode emit the next `break_count` ops, then skip
    /// `flat_count`; in flat mode skip `break_count` ops, then emit
    /// `flat_count` (law L6). Counts are u32 (ABI v2).
    IfBreak { break_count: u32, flat_count: u32 },
}

/// ABI v2's size bound. The 24-byte budget is part of the frozen layout
/// (`rust/ABI-v2.md`): a variant that grows the enum is a compile error here.
const _: () = assert!(std::mem::size_of::<FmtOp<'static>>() <= 24);
const _: () = assert!(std::mem::align_of::<FmtOp<'static>>() == 8);

/// Per-group metadata, one entry per `GroupStart`/`WrapStart`, in op order.
/// All fields are u32 (ABI v2): a group wider than 65,535 columns is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupHdr {
    /// Width of the group's flattened content ([W] §3 `flatten`), saturated
    /// at `u32::MAX`. Meaningless when `HAS_HARD` is set.
    pub flat_width: u32,
    /// Width of the rest after the group up to the end of its first line,
    /// as [W] §4 `fits` reads it (law L3′), saturated at `u32::MAX`.
    pub trailing: u32,
    /// Index of the group's closing op (`GroupEnd` or `WrapEnd`).
    pub end_pc: u32,
    /// `HAS_HARD`, `WRAP`.
    pub flags: u32,
}

impl GroupHdr {
    /// The group's flattening contains a hard line, so it can never be flat
    /// (law L2: `flatten hardline = Fail`, `fits _ SFail = False`).
    pub const HAS_HARD: u32 = 1 << 0;
    /// The header belongs to a `WrapStart`.
    pub const WRAP: u32 = 1 << 1;

    #[inline(always)]
    pub fn has_hard(&self) -> bool {
        self.flags & Self::HAS_HARD != 0
    }
}

const _: () = assert!(std::mem::size_of::<GroupHdr>() == 16);

/// A finished document: the op stream and its group-header table. Built once
/// by [`FmtBuilder::finish`](crate::FmtBuilder::finish), rendered any number
/// of times.
#[derive(Debug, Clone, Default)]
pub struct Doc<'a> {
    pub ops: Vec<FmtOp<'a>>,
    pub hdrs: Vec<GroupHdr>,
}

impl<'a> Doc<'a> {
    #[inline]
    pub fn ops(&self) -> &[FmtOp<'a>] {
        &self.ops
    }

    #[inline]
    pub fn hdrs(&self) -> &[GroupHdr] {
        &self.hdrs
    }
}
