use unchecked_unwrap::UncheckedUnwrap;
use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use db::Db;
use index::{Index, handle_all};
use crate::{predicate::one_where, is_null, filter::filter};

pub fn delete<'a>(d: &Delete<'a>, db: &mut Db) -> Result<'a, String> {
  unsafe {
    let (tp_id, tp) = db.get_tp(d.table)?;
    if db.has_foreign_link_to(tp_id) { return Err(AlterTableWithForeignLink(d.table)); }
    let pred = one_where(&d.where_, tp)?;
    let mut del_num = 0u32;
    filter(db.pr(), &d.where_, tp_id, tp.pr(), pred, |data, rid| {
      del_num += 1;
      for (idx, ci) in tp.cols().iter().enumerate() {
        if ci.index != !0 {
          macro_rules! handle {
            ($ty: ident) => {{
              let mut index = Index::<{ $ty }>::new(db, tp_id, idx as u32);
              if !is_null(data, idx as u32) {
                index.delete(data.add(ci.off as usize), rid);
              }
            }};
          }
          handle_all!(ci.ty.ty, handle);
        }
      }
      db.dealloc_data_slot(tp, rid);
      Ok(())
    }, false).unchecked_unwrap();
    tp.count -= del_num;
    Ok(format!("{} record(s) deleted", del_num))
  }
}