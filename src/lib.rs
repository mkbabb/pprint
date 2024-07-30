#![feature(let_chains)]
#![feature(iter_collect_into)]
#![feature(byte_slice_trim_ascii)]
#![feature(extend_one)]
#![feature(box_patterns)]
#![feature(stmt_expr_attributes)]

#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

pub mod doc;
pub use doc::*;

pub mod print;
pub use print::*;

pub mod utils;
pub use utils::*;

pub mod dtoa;
pub use dtoa::*;

extern crate pprint_derive;
pub use pprint_derive::*;
