use unchecked_unwrap::UncheckedUnwrap;

use common::{*, Error::*};
use syntax::ast::*;
use physics::*;
use db::{Db, fill_ptr};
use crate::{predicate::one_where, filter::filter};

pub fn update(u: &Update, db: &mut Db) -> Result<()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(u.table)?;
    let pred = one_where(&u.where_, u.table, tp)?;

    let buf = Align4U8::new(tp.size as usize); // this is only for error checking
    for &(col, val) in &u.sets {
      let ci = tp.get_ci(col)?.1;
      if ci.index != !0 { return Err(UpdateWithIndex(col.into())); }
      if ci.check != !0 { return Err(UpdateWithCheck(col.into())); }
      if val.is_null() {
        if ci.flags.contains(ColFlags::NOTNULL) { return Err(PutNullOnNotNull); }
      } else {
        fill_ptr(buf.ptr.add(ci.off as usize), ci.ty, val)?;
      }
    }

    filter(&u.where_, (tp_id, tp.prc()), db, pred, |data, _| {
      for &(col, val) in &u.sets {
        let (ci_id, ci) = tp.get_ci(col).unchecked_unwrap();
        debug_assert_eq!(ci.index, !0);
        if val.is_null() {
          debug_assert!(!ci.flags.contains(ColFlags::NOTNULL));
          *(data as *mut u32).add(ci_id / 32) |= 1 << ((ci_id % 32) as u32);
        } else {
          fill_ptr(data.add(ci.off as usize), ci.ty, val).unchecked_unwrap();
        }
      }
    });
    Ok(())
  }
}