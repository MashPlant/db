use common::{*, BareTy::*};
use syntax::ast::*;
use db::Db;
use index::{Index, handle_all, check_foreign_link};
use crate::{predicate::one_where, is_null, filter::filter};

pub fn delete<'a>(d: &Delete<'a>, db: &mut Db) -> ModifyResult<'a, u32> {
  unsafe {
    let (tp_id, tp) = db.get_tp(d.table)?;
    let f_links = db.foreign_links_to(tp_id);
    let pred = one_where(&d.where_, tp)?;
    let mut cnt = 0;
    if let Err(e) = filter(db.pr(), &d.where_, tp_id, pred, |data, rid| {
      check_foreign_link(db, tp, data, &f_links)?;
      for (ci_id, ci) in tp.cols().iter().enumerate() {
        if ci.index != !0 {
          macro_rules! handle {
            ($ty: ident) => {{
              let mut index = Index::<{ $ty }>::new(db, tp_id, ci_id as u32);
              if !is_null(data, ci_id as u32) { index.delete(data.add(ci.off as usize), rid); }
            }};
          }
          handle_all!(ci.ty.ty, handle);
        }
      }
      db.dealloc_data_slot(tp, rid);
      cnt += 1;
      tp.count -= 1;
      Ok(())
    }, false) { Err(ModifyError(cnt, e)) } else { Ok(cnt) }
  }
}