#![feature(ptr_offset_from)]
#![feature(box_syntax)]

pub mod db;
pub mod iter;
pub mod alter;
pub mod show;

pub use crate::{db::*, iter::*, show::*};

use regex::Regex;

use common::{*, Error::*, BareTy::*};
use chrono::NaiveDate;
use physics::ColInfo;

// `data` points to the beginning of the whole data slot
pub unsafe fn is_null(data: *const u8, ci_id: u32) -> bool { bsget(data as *const u32, ci_id as usize) }

// `ptr` points to the location in this record where `val` should locate, not the start address of data slot
// caller should guarantee `val` IS NOT NULL
// you can allocate some useless space for ptr to do error check
pub unsafe fn fill_ptr(ptr: *mut u8, ty: ColTy, val: CLit) -> Result<()> {
  match (ty.ty, val.lit()) {
    (_, Lit::Null) => debug_unreachable!(),
    (Bool, Lit::Bool(v)) => (ptr as *mut bool).write(v),
    (Int, Lit::Number(v)) => (ptr as *mut i32).write(v as i32),
    (Float, Lit::Number(v)) => (ptr as *mut f32).write(v as f32),
    (Date, Lit::Str(v)) => (ptr as *mut NaiveDate).write(date(v)?),
    (Date, Lit::Date(v)) => (ptr as *mut NaiveDate).write(v), // it is not likely to enter this case, because parser cannot produce Date
    (VarChar, Lit::Str(v)) => {
      if v.len() > ty.size as usize { return Err(PutStrTooLong { limit: ty.size, actual: v.len() }); }
      ptr.write(v.len() as u8);
      ptr.add(1).copy_from_nonoverlapping(v.as_ptr(), v.len());
    }
    (expect, val) => return Err(RecordLitTyMismatch { expect, actual: val.ty() })
  }
  Ok(())
}

// input the whole data slot, result may be null
pub unsafe fn data2lit<'a>(data: *const u8, ci_id: u32, ci: &ColInfo) -> CLit<'a> {
  if bsget(data as *const u32, ci_id as usize) { return CLit::new(Lit::Null); };
  ptr2lit(data.add(ci.off as usize), ci.ty.ty)
}

// input the data ptr, result is never null
pub unsafe fn ptr2lit<'a>(ptr: *const u8, ty: BareTy) -> CLit<'a> {
  CLit::new(match ty {
    Bool => Lit::Bool(*(ptr as *const bool)),
    Int => Lit::Number(*(ptr as *const i32) as f64),
    Float => Lit::Number(*(ptr as *const f32) as f64),
    Date => Lit::Date(*(ptr as *const NaiveDate)),
    VarChar => Lit::Str(str_from_db(ptr)),
  })
}

pub fn date(date: &str) -> Result<NaiveDate> {
  NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|reason| InvalidDate { date, reason })
}

pub fn like2re(like: &str) -> Result<Regex> {
  Regex::new(&escape_re(like)).map_err(|e| InvalidLike { like, reason: box e })
}

pub unsafe fn hash_pks(data: *const u8, pks: &[&ColInfo]) -> u128 {
  const SEED: u128 = 19260817;
  let mut hash = 0u128;
  for &col in pks {
    let ptr = data.add(col.off as usize);
    match col.ty.ty {
      Bool => hash = hash.wrapping_mul(SEED).wrapping_add(*ptr as u128),
      Int | Float | Date => hash = hash.wrapping_mul(SEED).wrapping_add(*(ptr as *const u32) as u128),
      VarChar => for &b in str_from_db(ptr).as_bytes() { hash = hash.wrapping_mul(SEED).wrapping_add(b as u128); }
    }
  }
  hash
}

fn escape_re(like: &str) -> String {
  let mut re = String::with_capacity(like.len());
  let mut escape = false;
  macro_rules! push {
    ($ch: expr) => {{
      if regex_syntax::is_meta_character($ch) { re.push('\\'); }
      re.push($ch);
    }};
  }
  for ch in like.chars() {
    if escape {
      match ch {
        '%' | '_' => re.push(ch), // \% => %, \_ => \_
        _ => { // \\ => \\, // \other => \\maybe_escape(other)
          if ch != '\\' { push!('\\'); }
          push!(ch);
        }
      }
      escape = false;
    } else {
      match ch {
        '\\' => escape = true,
        '%' => { (re.push('.'), re.push('*')); }
        '_' => re.push('.'),
        _ => push!(ch),
      }
    }
  }
  if escape { push!('\\'); }
  re
}

#[test]
fn test_escape() {
  assert_eq!(escape_re(r#"%_"#), r#".*."#);
  assert_eq!(escape_re(r#"%_\%\_\\"#), r#".*.%_\\"#);
  assert_eq!(escape_re(r#"\n\r\t\\\"#), r#"\\n\\r\\t\\\\"#);
  assert_eq!(escape_re(r#".*."#), r#"\.\*\."#);
}