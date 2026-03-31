// ── FmtOp flat buffer system (primary formatting path) ──────────────────────
pub mod op;
pub mod builder;
pub mod render;

pub use op::FmtOp;
pub use builder::FmtBuilder;
pub use render::{render, render_with_hint};

// ── Shared utilities ────────────────────────────────────────────────────────
pub mod utils;
pub use utils::*;

pub mod dtoa;
pub use dtoa::*;

// ── Doc tree (used by Pretty derive macro + VM interpreter) ─────────────────
pub mod doc;
pub use doc::*;

pub mod print;
pub use print::*;

pub use pprint_derive::*;
