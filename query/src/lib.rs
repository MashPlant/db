#![feature(ptr_offset_from)]
#![feature(box_patterns)]

pub mod insert;
pub mod delete;
pub mod select;
pub mod update;
mod predicate;
mod filter;

pub use crate::{insert::*, delete::*, select::*, update::*};

// `data` points to the begining of the whole data slot
pub(crate) unsafe fn is_null(data: *const u8, ci_id: u32) -> bool {
  common::bsget(data as *const u32, ci_id as usize)
}