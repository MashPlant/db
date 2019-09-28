use common::{*, Error::*, BareTy::*};
use db::Db;
use physics::Rid;
use crate::{Index, handle_all};

// place it here instead of in Db because need to insert all existing keys
pub fn create(db: &mut Db, table: &str, col: &str) -> Result<()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(table)?;
    let (ci_id, ci) = tp.get_ci(col)?;
    if ci.index != !0 { return Err(DupIndex(col.into())); }
    db.alloc_index(ci);
    macro_rules! handle {
      ($ty: ident) => {{
        let mut index = Index::<{ $ty }>::new(db, Rid::new(tp_id as u32, ci_id as u32));
        for (data, rid) in db.record_iter((tp_id, tp)) {
          let data = data.as_ptr();
          let ptr = data.add(ci.off as usize);
          if !bsget(data as *const u32, ci_id) { // not null
            index.insert(ptr, rid);
          }
        }
      }};
    }
    handle_all!(ci.ty.ty, handle);
    Ok(())
  }
}