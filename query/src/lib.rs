#![feature(ptr_offset_from)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(inner_deref)]

pub mod insert;
pub mod delete;
pub mod select;
pub mod update;
mod predicate;
mod filter;

pub use crate::{insert::*, delete::*, select::*, update::*};

use db::{Db, is_null, ptr2lit};
use physics::*;
use index::{Index, handle_all};
use common::{*, Error::*, BareTy::*};

// return Err if there is a foreign link to `data`
unsafe fn check_foreign_link<'a>(db: &Db, tp: &TablePage, data: *const u8, f_links: &[(u32, u8, u8)]) -> Result<'a, ()> {
  let db = db.pr();
  for &(tp_id1, ci_id1, ci_id) in f_links {
    let ci = tp.cols.get_unchecked(ci_id as usize);
    let ptr = data.add(ci.off as usize);
    macro_rules! handle {
      ($ty: ident) => {{
        if !is_null(data, ci_id as u32) && Index::<{ $ty }>::new(db, tp_id1, ci_id1 as u32).contains(ptr) {
          return Err(ModifyColWithForeignLink { col: ci.name(), val: ptr2lit(ptr, $ty) });
        }
      }};
    }
    handle_all!(ci.ty.ty, handle);
  }
  Ok(())
}
