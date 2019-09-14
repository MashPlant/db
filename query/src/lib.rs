#![feature(ptr_offset_from)]

pub mod insert;
pub mod delete;
pub mod select;
pub mod update;
mod predicate;
mod filter;

pub use crate::{insert::*, delete::*, select::*, update::*};

use chrono::NaiveDate;

use common::{*, BareTy::*, Error::*};

// null bitset is in the header part of a data slot
#[inline(always)]
pub(crate) unsafe fn is_null(p: *const u8, idx: usize) -> bool {
  ((*(p as *const u32).add(idx / 32) >> (idx as u32 % 32)) & 1) != 0
}

// `ptr` points to the location in this record where `val` should locate, not the start address of data slot
// caller should guarantee `val` IS NOT NULL
#[inline(always)]
pub(crate) unsafe fn fill_ptr(ptr: *mut u8, col_ty: ColTy, val: Lit) -> Result<()> {
  match (col_ty, val) {
    (ColTy { .. }, Lit::Null) => debug_unreachable!(),
    // no implicit cast is allowed
    (ColTy { ty: Int, .. }, Lit::Int(v)) => (ptr as *mut i32).write(v),
    (ColTy { ty: Bool, .. }, Lit::Bool(v)) => (ptr as *mut bool).write(v),
    (ColTy { ty: Float, .. }, Lit::Float(v)) => (ptr as *mut f32).write(v),
    (ColTy { ty: Char, size }, Lit::Str(v)) | (ColTy { ty: VarChar, size }, Lit::Str(v)) => {
      if v.len() > size as usize { return Err(PutStrTooLong { limit: size, actual: v.len() }); }
      ptr.write(v.len() as u8);
      ptr.add(1).copy_from_nonoverlapping(v.as_ptr(), v.len());
    }
    (ColTy { ty: Date, .. }, Lit::Str(v)) => match NaiveDate::parse_from_str(v, "%Y-%m-%d") {
      Ok(date) => (ptr as *mut NaiveDate).write(date),
      Err(reason) => return Err(InvalidDate { date: (*v).into(), reason })
    }
    _ => return Err(RecordLitTyMismatch { expect: col_ty.ty, actual: val.ty() })
  }
  Ok(())
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