#![feature(ptr_offset_from)]

pub mod db;
pub mod iter;
pub mod show;

pub use crate::{db::*, iter::*, show::*};

use common::{*, Error::*, BareTy::*};
use chrono::NaiveDate;
use physics::ColInfo;

// `ptr` points to the location in this record where `val` should locate, not the start address of data slot
// caller should guarantee `val` IS NOT NULL
// you can allocate some useless space for ptr to do error check
pub unsafe fn fill_ptr(ptr: *mut u8, col: ColTy, val: CLit) -> Result<()> {
  match (col.ty, val.lit()) {
    (_, Lit::Null) => debug_unreachable!(),
    (Bool, Lit::Bool(v)) => (ptr as *mut bool).write(v),
    (Int, Lit::Number(v)) => (ptr as *mut i32).write(v as i32),
    (Float, Lit::Number(v)) => (ptr as *mut f32).write(v as f32),
    (Date, Lit::Str(v)) => match NaiveDate::parse_from_str(v, "%Y-%m-%d") {
      Ok(date) => (ptr as *mut NaiveDate).write(date),
      Err(reason) => return Err(InvalidDate { date: v, reason })
    }
    (Date, Lit::Date(v)) => (ptr as *mut NaiveDate).write(v), // it is not likely to enter this case, because parser cannot produce Date
    (VarChar, Lit::Str(v)) => {
      let size = col.size;
      if v.len() > size as usize { return Err(PutStrTooLong { limit: size, actual: v.len() }); }
      ptr.write(v.len() as u8);
      ptr.add(1).copy_from_nonoverlapping(v.as_ptr(), v.len());
    }
    (expect, val) => return Err(RecordLitTyMismatch { expect, actual: val.ty() })
  }
  Ok(())
}

pub unsafe fn ptr2lit(data: *const u8, ci_id: u32, ci: &ColInfo) -> CLit {
  if bsget(data as *const u32, ci_id as usize) { return CLit::new(Lit::Null); };
  let ptr = data.add(ci.off as usize);
  CLit::new(match ci.ty.ty {
    Bool => Lit::Bool(*(ptr as *const bool)),
    Int => Lit::Number(*(ptr as *const i32) as f64),
    Float => Lit::Number(*(ptr as *const f32) as f64),
    Date => Lit::Date(*(ptr as *const NaiveDate)),
    VarChar => Lit::Str(str_from_db(ptr)),
  })
}