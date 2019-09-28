#![feature(ptr_offset_from)]

pub mod insert;
pub mod delete;
pub mod select;
pub mod update;
mod predicate;
mod filter;

pub use crate::{insert::*, delete::*, select::*, update::*};

// null bitset is in the header part of a data slot
pub(crate) unsafe fn is_null(p: *const u8, idx: usize) -> bool { common::bsget(p as *const u32, idx) }