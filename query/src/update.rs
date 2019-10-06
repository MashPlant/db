use unchecked_unwrap::UncheckedUnwrap;

use common::{*, Error::*};
use syntax::ast::*;
use physics::*;
use db::{Db, fill_ptr};
use crate::{predicate::one_where, filter::filter};

pub fn update<'a>(u: &Update<'a>, db: &mut Db) -> Result<'a, String> {
  unsafe {
    let (tp_id, tp) = db.get_tp(u.table)?;
    let pred = one_where(&u.where_, u.table, tp)?;

    let buf = Align4U8::new(tp.size as usize); // this is only for error checking
    for &(col, val) in &u.sets {
      let ci = tp.get_ci(col)?;
      if ci.index != !0 { return Err(UpdateWithIndex(col)); }
      if ci.check != !0 { return Err(UpdateWithCheck(col)); }
      if val.is_null() {
        if ci.flags.contains(ColFlags::NOTNULL) { return Err(PutNullOnNotNull); }
      } else {
        fill_ptr(buf.ptr.add(ci.off as usize), ci.ty, val)?;
      }
    }

    let mut update_num = 0;
    filter(&u.where_, (tp_id, tp.prc()), db, pred, |data, _| {
      update_num += 1;
      for &(col, val) in &u.sets {
        let ci = tp.get_ci(col).unchecked_unwrap();
        let ci_id = ci.idx(&tp.cols);
        debug_assert_eq!(ci.index, !0);
        if val.is_null() {
          debug_assert!(!ci.flags.contains(ColFlags::NOTNULL));
          bsset(data as *mut u32, ci_id as usize);
        } else {
          bsdel(data as *mut u32, ci_id as usize); // now not null (no matter whether it is null before)
          fill_ptr(data.add(ci.off as usize), ci.ty, val).unchecked_unwrap();
        }
      }
    });
    Ok(format!("{} record(s) updated", update_num))
  }
}