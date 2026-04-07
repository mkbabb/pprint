// ── FmtOp flat buffer system ────────────────────────────────────────────────
pub mod op;
pub mod builder;
pub mod render;

pub use op::FmtOp;
pub use builder::FmtBuilder;
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

// ── Serializer integration ────────────────────────────────────────────────
#[cfg(feature = "ser")]
mod serializer;
