use common::{*, BareTy::*};
use syntax::ast::*;
use db::{Db, is_null};
use index::{Index, handle_all};
use crate::{predicate::one_where, filter::filter, check_foreign_link};

pub fn delete<'a>(d: &Delete<'a>, db: &mut Db) -> ModifyResult<'a, u32> {
  unsafe {
    let (tp_id, tp) = db.get_tp(d.table)?;
    let f_links = db.foreign_links_to(tp_id).collect::<Vec<_>>();
    let pred = one_where(db.pr(), &d.where_, tp)?;
    let mut cnt = 0;
    if let Err(e) = filter(db.pr(), &d.where_, tp_id, pred, |data, rid| {
      check_foreign_link(db, tp, data, &f_links)?;
      // now no error can occur
      for (ci_id, ci) in tp.cols().iter().enumerate() {
        let (ci_id, ptr) = (ci_id as u32, data.add(ci.off as usize));
        if !is_null(data, ci_id) {
          if ci.index != !0 {
            macro_rules! handle { ($ty: ident) => {{ Index::<{ $ty }>::new(db, tp_id, ci_id).delete(ptr, rid); }}; }
            handle_all!(ci.ty.fix_ty().ty, handle);
          }
          if ci.ty.is_varchar() { db.free_varchar(ptr); }
        }
      }
      db.dealloc_data_slot(tp, rid);
      cnt += 1;
      tp.count -= 1;
      Ok(())
    }, false) { Err(ModifyError(cnt, e)) } else { Ok(cnt) }
  }
}