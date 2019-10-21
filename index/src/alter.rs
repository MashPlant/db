use unchecked_unwrap::UncheckedUnwrap;

use common::{*, Error::*, BareTy::*};
use db::{Db, ptr2lit};
use syntax::*;
use physics::*;
use crate::{Index, handle_all};

// some alter operation cannot be put in `db` crate, because the need some index operation, and `index` crate depends on `db` crate

// though we specify an index with a length = 0 means an internal index, it is not checked here
// (you can create such an index though manually constructing `CreateIndex` struct, but not through parser)
// mainly because even if you did that, there is no serious consequence
pub fn create_index<'a>(db: &mut Db, c: &CreateIndex<'a>) -> Result<'a, ()> {
  unsafe {
    for &tp_id in db.dp().tables() {
      for ci in db.get_page::<TablePage>(tp_id).cols() {
        if ci.idx_name().filter(|&x| x == c.index).is_some() { return Err(DupIndex(c.index)); }
      }
    }
    let (tp_id, tp) = db.get_tp(c.table)?;
    let ci = tp.get_ci(c.col)?;
    if ci.index == !0 {
      db.alloc_index(ci, c.index)?;
      insert_all(db, tp_id, tp, ci);
    }
    Ok(())
  }
}

pub fn drop_table<'a>(db: &mut Db, table: &'a str) -> Result<'a, ()> {
  unsafe {
    let dp = db.dp();
    for (idx, &tp_id) in dp.tables().iter().enumerate() {
      let tp = db.get_page::<TablePage>(tp_id);
      if tp.name() == table {
        let f_links = db.foreign_links_to(tp_id);
        for (data, _) in db.pr().record_iter(tp) {
          check_foreign_link(db, tp, data.as_ptr(), &f_links)?;
        }
        let tables = dp.tables.as_mut_ptr();
        tables.add(idx).swap(tables.add(dp.table_num as usize - 1));
        dp.table_num -= 1;
        tp.cols().iter().filter(|ci| ci.index != !0).for_each(|ci| db.dealloc_index(ci.pr()));
        let mut cur = tp.first;
        while cur != !0 {
          let next = db.get_page::<DataPage>(cur).next;
          db.dealloc_page(cur);
          cur = next;
        }
        return Ok(());
      }
    }
    Err(NoSuchTable(table))
  }
}

pub fn add_foreign<'a>(db: &mut Db, a: &AddForeign<'a>) -> Result<'a, ()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(a.table)?;
    let ci = tp.get_ci(a.col)?;
    let ci_id = ci.idx(&tp.cols);
    if ci.f_table != !0 { return Err(DupConstraint(a.col)); }
    let (f_tp_id, f_tp) = db.get_tp(a.f_table)?;
    let f_ci = f_tp.get_ci(a.f_col)?;
    let f_ci_id = f_ci.idx(&f_tp.cols);
    if !f_ci.flags.contains(ColFlags::UNIQUE) { return Err(ForeignOnNotUnique(a.f_col)); }
    if f_ci.ty != ci.ty { return Err(IncompatibleForeignTy { foreign: f_ci.ty, own: ci.ty }); };
    macro_rules! handle {
      ($ty: ident) => {{
        let index = Index::<{ $ty }>::new(db, f_tp_id, f_ci_id);
        for (data, _) in db.record_iter(tp) {
          let ptr = data.as_ptr().add(ci.off as usize);
          if !bsget(data.as_ptr() as *const u32, ci_id as usize) && !index.contains(ptr) {
            return Err(PutNonexistentForeign { col: a.col, val: ptr2lit(ptr, $ty) });
          }
        }
      }};
    }
    handle_all!(ci.ty.ty, handle);
    // now no error can occur
    (ci.f_table = f_tp_id, ci.f_col = f_ci_id as u8);
    if ci.index == !0 {
      db.alloc_index(ci, "").unchecked_unwrap();
      insert_all(db, tp_id, tp, ci);
    }
    Ok(())
  }
}

unsafe fn insert_all(db: &mut Db, tp_id: u32, tp: &TablePage, ci: &ColInfo) {
  let ci_id = ci.idx(&tp.cols);
  macro_rules! handle {
    ($ty: ident) => {{
      let mut index = Index::<{ $ty }>::new(db, tp_id, ci_id);
      for (data, rid) in db.record_iter(tp) {
        if !bsget(data.as_ptr() as *const u32, ci_id as usize) { // not null
          index.insert(data.as_ptr().add(ci.off as usize), rid);
        }
      }
    }};
  }
  handle_all!(ci.ty.ty, handle);
}

// return Err if there is a foreign link to `data`; the return value's life time is the same as `data`
pub unsafe fn check_foreign_link<'a>(db: &Db, tp: &TablePage, data: *const u8, f_links: &[(u32, u8, u8)]) -> Result<'a, ()> {
  let db = db.pr();
  for &(tp_id1, ci_id1, ci_id) in f_links {
    let ci = tp.cols.get_unchecked(ci_id as usize);
    let ptr = data.add(ci.off as usize);
    macro_rules! handle {
      ($ty: ident) => {{
        if !bsget(data as *const u32, ci_id as usize) && Index::<{ $ty }>::new(db, tp_id1, ci_id1 as u32).contains(ptr) {
          return Err(ModifyColWithForeignLink { col: ci.name(), val: ptr2lit(ptr, $ty) });
        }
      }};
    }
    handle_all!(ci.ty.ty, handle);
  }
  Ok(())
}