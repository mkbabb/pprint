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

    /// Emit a single printable ASCII byte (delimiters, operators, commas).
    /// The payload is an [`AsciiByte`], so the byte is valid UTF-8 and one
    /// display column by construction (LW3.PP-1a).
    Char(AsciiByte),

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
    WrapStart { open: AsciiByte, hdr: u32 },

    /// End a wrapped container: indent_end + break_line + close char + group_end.
    /// Combines IndentEnd + BreakLine + Char(close) + GroupEnd into one op.
    WrapEnd { close: AsciiByte },

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
        flat: [AsciiByte; 8],
        flat_len: u8,
        brk: [AsciiByte; 4],
        brk_len: u8,
    },

    /// [P] `flatAlt`: in break mode emit the next `break_count` ops, then skip
    /// `flat_count`; in flat mode skip `break_count` ops, then emit
    /// `flat_count` (law L6). Counts are u32 (ABI v2).
    IfBreak { break_count: u32, flat_count: u32 },

    // ── Fill and trivia (sk-v27 LW3.PP-2, `ABI-v2.md` LW3 addendum) ─────
    /// A fill separator, [L] `softline = group line` generalized to `k =
    /// spaces` spaces: `group (flatAlt linebreak (text (spaces k)))` (law
    /// L5, ruling S8-B). `hdr` indexes the [`GroupHdr`] table like a
    /// group's, with `flat_width = spaces`; the separator decides itself at
    /// render time, so the trailing of the item before it stops here
    /// (L3′(c)).
    FillSep { hdr: u32, spaces: u32 },

    /// A line suffix (law L9): the next `len` ops (leaves only) are held
    /// back and emitted just before the next structural newline, or at the
    /// end of the document. They count as zero width.
    LineSuffix { len: u32 },

    /// If a line suffix is pending, a hard line that flushes it (L9);
    /// otherwise nothing.
    LineSuffixBoundary,

    /// No output; forces the enclosing group and every group around it to
    /// break, like a hard line, except across an `IfBreak` break branch
    /// (law L2).
    BreakParent,
}

/// A printable ASCII byte, `0x20..=0x7E` (sk-v27 LW3.PP-1a, `ABI-v2.md`
/// LW3 addendum). The type of every byte the op stream carries outside a
/// `&str`: `Char`, the delimiters of `WrapStart`/`WrapEnd` and the bytes of
/// `Sep`.
///
/// It is valid UTF-8 on its own, so the renderer builds its output `String`
/// from `&str` and `AsciiByte` pushes with no unchecked conversion, and it is
/// exactly one display column, so a width is never a guess. A non-ASCII byte,
/// a tab, a line feed or any other control byte has no `AsciiByte`: the
/// builder refuses it at `finish()` ([`BuildError::NotAscii`]).
///
/// [`BuildError::NotAscii`]: crate::BuildError::NotAscii
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AsciiByte(u8);

impl AsciiByte {
    /// The space, `0x20`.
    pub const SPACE: AsciiByte = AsciiByte(b' ');

    /// `Some` iff `b` is printable ASCII (`0x20..=0x7E`).
    #[inline]
    pub const fn new(b: u8) -> Option<AsciiByte> {
        if b >= 0x20 && b <= 0x7E {
            Some(AsciiByte(b))
        } else {
            None
        }
    }

    /// The byte.
    #[inline(always)]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The byte as a `char` (always a one-byte, one-column scalar).
    #[inline(always)]
    pub const fn as_char(self) -> char {
        self.0 as char
    }
}

impl TryFrom<u8> for AsciiByte {
    type Error = u8;

    /// The byte itself is the error when it is not printable ASCII.
    #[inline]
    fn try_from(b: u8) -> Result<Self, u8> {
        AsciiByte::new(b).ok_or(b)
    }
}

const _: () = assert!(std::mem::size_of::<AsciiByte>() == 1);

/// ABI v2's size bound. The 24-byte budget is part of the frozen layout
/// (`rust/ABI-v2.md`): a variant that grows the enum is a compile error here.
const _: () = assert!(std::mem::size_of::<FmtOp<'static>>() <= 24);
const _: () = assert!(std::mem::align_of::<FmtOp<'static>>() == 8);

/// Per-group metadata, one entry per `GroupStart`/`WrapStart`/`FillSep`, in
/// op order. All fields are u32 (ABI v2): a group wider than 65,535 columns
/// is exact.
///
/// The decision reads the pending-suffix state `π` (law L9): a
/// `LineSuffixBoundary` with a suffix pending is a newline, so the width
/// `fits` reads depends on whether a suffix is pending when the group opens.
/// `cut` and `cut_pending` give the width up to the first such boundary in
/// the group's flattening (`NO_CUT` if none); `trailing` and
/// `trailing_pending` give the rest after the group under each state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupHdr {
    /// Width of the group's flattened content ([W] §3 `flatten`), saturated
    /// at `u32::MAX`. Meaningless when `HAS_HARD` is set.
    pub flat_width: u32,
    /// Width of the rest after the group up to the end of its first line,
    /// as [W] §4 `fits` reads it (law L3′), with no suffix pending after the
    /// group; saturated at `u32::MAX`.
    pub trailing: u32,
    /// As `trailing`, with a suffix pending after the group (L9).
    pub trailing_pending: u32,
    /// With no suffix pending at the open: the flat width up to the first
    /// `LineSuffixBoundary` of the flattening that a suffix inside the
    /// group makes a newline, or [`GroupHdr::NO_CUT`].
    pub cut: u32,
    /// With a suffix pending at the open: the flat width up to the first
    /// `LineSuffixBoundary` of the flattening, or [`GroupHdr::NO_CUT`].
    pub cut_pending: u32,
    /// Index of the group's closing op (`GroupEnd` or `WrapEnd`; the op
    /// itself for a `FillSep`).
    pub end_pc: u32,
    /// `HAS_HARD`, `WRAP`, `FILL`, `SUFFIX`.
    pub flags: u32,
}

impl GroupHdr {
    /// The group's flattening contains a hard line (a `Hardline`, a
    /// multi-line leaf or a `BreakParent`), so it can never be flat (law L2:
    /// `flatten hardline = Fail`; [P] `group` of a never-flat document).
    pub const HAS_HARD: u32 = 1 << 0;
    /// The header belongs to a `WrapStart`.
    pub const WRAP: u32 = 1 << 1;
    /// The header belongs to a `FillSep`.
    pub const FILL: u32 = 1 << 2;
    /// The group's flattening holds a `LineSuffix` (and no boundary that
    /// it would turn into a newline), so a suffix is pending after it.
    pub const SUFFIX: u32 = 1 << 3;
    /// No boundary cuts the flattening.
    pub const NO_CUT: u32 = u32::MAX;

    #[inline(always)]
    pub fn has_hard(&self) -> bool {
        self.flags & Self::HAS_HARD != 0
    }
}

const _: () = assert!(std::mem::size_of::<GroupHdr>() == 28);
const _: () = assert!(std::mem::align_of::<GroupHdr>() == 4);

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
