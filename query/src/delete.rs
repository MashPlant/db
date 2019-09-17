use common::{*, BareTy::*};
use syntax::ast::*;
use physics::*;
use db::Db;
use index::Index;
use crate::{handle_all, predicate::one_where, is_null, filter::filter};

pub fn delete(d: &Delete, db: &mut Db) -> Result<()> {
  unsafe {
    let tp = db.get_tp(d.table)?;
    let table_page = db.id_of(tp);
    let pred = one_where(&d.where_, d.table, tp)?;
    let mut del = Vec::new();
    filter(&d.where_, tp, db, |data| pred(data), |data, rid| del.push((data, rid)));
    for (idx, ci) in tp.cols().iter().enumerate() {
      if ci.index != !0 {
        macro_rules! handle {
          ($ty: ident) => {{
            let mut index = Index::<{ $ty }>::new(db, Rid::new(table_page as u32, idx as u32));
            for &(data, rid) in &del {
              if !is_null(data, idx) { // null item doesn't get deleted from index
                index.delete(data.add(ci.off as usize), rid);
              }
            }
          }};
        }
        handle_all!(ci.ty.ty, handle);
      }
    }
    for &(_, rid) in &del { db.deallocate_data_slot(tp, rid); }
    Ok(())
  }
}