#![feature(ptr_offset_from)]

pub mod db;
pub mod iter;
pub mod show;

pub use crate::{db::*, iter::*, show::*};

use common::{*, Error::*, BareTy::*};
use chrono::NaiveDate;

// `ptr` points to the location in this record where `val` should locate, not the start address of data slot
// caller should guarantee `val` IS NOT NULL
// you can allocate some useless space for ptr to do error check
#[inline(always)]
pub unsafe fn fill_ptr(ptr: *mut u8, col: ColTy, val: Lit) -> Result<()> {
  match (col.ty, val) {
    (_, Lit::Null) => debug_unreachable!(),
    (Int, Lit::Int(v)) => (ptr as *mut i32).write(v),
    (Bool, Lit::Bool(v)) => (ptr as *mut bool).write(v),
    (Float, Lit::Float(v)) => (ptr as *mut f32).write(v),
    (Int, Lit::Float(v)) => (ptr as *mut i32).write(v as i32),
    (Float, Lit::Int(v)) => (ptr as *mut f32).write(v as f32),
    (Char, Lit::Str(v)) | (VarChar, Lit::Str(v)) => {
      let size = col.size;
      if v.len() > size as usize { return Err(PutStrTooLong { limit: size, actual: v.len() }); }
      ptr.write(v.len() as u8);
      ptr.add(1).copy_from_nonoverlapping(v.as_ptr(), v.len());
    }
    (Date, Lit::Str(v)) => match NaiveDate::parse_from_str(v, "%Y-%m-%d") {
      Ok(date) => (ptr as *mut NaiveDate).write(date),
      Err(reason) => return Err(InvalidDate { date: (*v).into(), reason })
    }
    _ => return Err(RecordLitTyMismatch { expect: col.ty, actual: val.ty() })
  }
  Ok(())
}