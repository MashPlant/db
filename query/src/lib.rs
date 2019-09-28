#![feature(ptr_offset_from)]

pub mod insert;
pub mod delete;
pub mod select;
pub mod update;
mod predicate;
mod filter;

pub use crate::{insert::*, delete::*, select::*, update::*};

// null bitset is in the header part of a data slot
pub(crate) unsafe fn is_null(p: *const u8, idx: usize) -> bool {
  ((*(p as *const u32).add(idx / 32) >> (idx % 32)) & 1) != 0
}

#[macro_use]
pub(crate) mod macros {
  // handle all kinds of Index with regard to different types
  #[macro_export]
  macro_rules! handle_all {
    ($ty: expr, $handle: ident) => {
      match $ty { Int => $handle!(Int), Bool => $handle!(Bool), Float => $handle!(Float), Char => $handle!(Char), VarChar => $handle!(VarChar), Date => $handle!(Date) }
    };
  }
}