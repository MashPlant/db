use common::{*, Error::*, BareTy::*};
use db::Db;
use physics::*;
use crate::{Index, handle_all};

// place it here instead of in Db because need to insert all existing keys
pub fn create<'a>(db: &mut Db, table: &'a str, col: &'a str) -> Result<'a, ()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(table)?;
    let ci = tp.get_ci(col)?;
    let ci_id = ci.idx(&tp.cols);
    if ci.index != !0 { return Err(DupIndex(col)); }
    db.alloc_index(ci);
    macro_rules! handle {
      ($ty: ident) => {{
        let mut index = Index::<{ $ty }>::new(db, Rid::new(tp_id, ci_id));
        for (data, rid) in db.record_iter(tp) {
          let data = data.as_ptr();
          let ptr = data.add(ci.off as usize);
          if !bsget(data as *const u32, ci_id as usize) { // not null
            index.insert(ptr, rid);
          }
        }
      }};
    }
    handle_all!(ci.ty.ty, handle);
    Ok(())
  }
}