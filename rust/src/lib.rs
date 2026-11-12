#![forbid(unsafe_code)]

// ── FmtOp flat buffer system ────────────────────────────────────────────────
pub mod builder;
mod layout;
pub mod op;
pub mod render;

pub use builder::FmtBuilder;
pub use layout::BuildError;
pub use op::{AsciiByte, Doc, FmtOp, GroupHdr};
pub use render::{PRINTER, Printer, render, render_with_hint};

// ── Pretty trait + standard type impls ──────────────────────────────────────
pub mod pretty;
pub use pretty::{Pretty, pprint, pprint_with_hint};

// ── Shared utilities ────────────────────────────────────────────────────────
pub mod utils;
pub use utils::*;

pub mod dtoa;
pub use dtoa::*;

pub use pprint_derive::*;
