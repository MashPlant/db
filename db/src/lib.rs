#![feature(ptr_offset_from)]
#![feature(box_syntax)]

pub mod db;
pub mod iter;
pub mod alter;
pub mod show;
pub mod lob;

pub use crate::{db::*, iter::*, lob::*, show::*};

use regex::Regex;

use common::{*, Error::*, BareTy::*};
use chrono::NaiveDate;
use physics::ColInfo;

// `data` points to the beginning of the whole data slot
pub unsafe fn is_null(data: *const u8, ci_id: u32) -> bool { bsget(data as *const u32, ci_id as usize) }

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
    match col.ty.fix_ty().ty {
      Bool => hash = hash.wrapping_mul(SEED).wrapping_add(*ptr as u128),
      Int | Float | Date => hash = hash.wrapping_mul(SEED).wrapping_add(*(ptr as *const u32) as u128),
      Char => for &b in str_from_db(ptr).as_bytes() { hash = hash.wrapping_mul(SEED).wrapping_add(b as u128); }
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