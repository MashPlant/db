use unchecked_unwrap::UncheckedUnwrap;

use common::{*, Error::*, BareTy::*};
use db::{Db, is_null, hash_pks};
use syntax::ast::*;
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
    if ci.ty.is_varchar() { return Err(UnsupportedVarcharOp(c.col)); }
    if ci.index == !0 {
      db.alloc_index(ci, c.index)?;
      insert_all(db, tp_id, tp, ci);
    }
    Ok(())
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
    if !f_ci.unique(f_tp.primary_cols().count()) { return Err(ForeignOnNotUnique(a.f_col)); }
    debug_assert!(!f_ci.ty.is_varchar());
    if f_ci.ty != ci.ty { return Err(IncompatibleForeignTy { foreign: f_ci.ty, own: ci.ty }); }
    macro_rules! handle {
      ($ty: ident) => {{
        let index = Index::<{ $ty }>::new(db, f_tp_id, f_ci_id);
        for (data, _) in db.record_iter(tp) {
          let ptr = data.add(ci.off as usize);
          if !is_null(data, ci_id) && !index.contains(ptr) {
            return Err(PutNonexistentForeign { col: a.col, val: db.ptr2lit(ptr, ci.ty) });
          }
        }
      }};
    }
    handle_all!(ci.ty.fix_ty().ty, handle);
    // now no error can occur
    (ci.f_table = f_tp_id, ci.f_col = f_ci_id as u8);
    if ci.index == !0 {
      db.alloc_index(ci, "").unchecked_unwrap();
      insert_all(db, tp_id, tp, ci);
    }
    Ok(())
  }
}

pub fn add_primary<'a>(db: &mut Db, table: &'a str, cols: &[&'a str]) -> Result<'a, ()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(table)?;
    let mut pks = tp.primary_cols().collect::<Vec<_>>();
    let old_len = pks.len();
    pks.reserve(cols.len());
    for (idx, &col) in cols.iter().enumerate() {
      if cols.iter().take(idx).any(|&x| x == col) { return Err(DupCol(col)); }
      let ci = tp.get_ci(col)?;
      if ci.flags.contains(ColFlags::PRIMARY) { return Err(DupConstraint(col)); }
      if ci.ty.is_varchar() { return Err(UnsupportedVarcharOp(col)); }
      pks.push(ci);
    }
    for (data, _) in db.record_iter(tp) {
      for &ci in pks.get_unchecked(old_len..) {
        if is_null(data, ci.idx(&tp.cols)) { return Err(PutNullOnNotNull); }
      }
    }
    if old_len == 0 && pks.len() != 0 { check_dup(db, tp, &pks)?; }
    for (_, _, ci_id) in db.foreign_links_to(tp_id) {
      let ci = tp.cols.get_unchecked(ci_id as usize);
      if !ci.unique(pks.len()) { return Err(ForeignOnNotUnique(ci.name())); }
    }
    // now no error can occur
    for &ci in pks.get_unchecked(old_len..) { ci.pr().flags.set(ColFlags::PRIMARY, true); }
    index_unique_primary(db, tp_id, tp);
    Ok(())
  }
}

pub fn drop_primary<'a>(db: &mut Db, table: &'a str, cols: &[&'a str]) -> Result<'a, ()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(table)?;
    let mut pks = tp.primary_cols().collect::<Vec<_>>();
    let mut new_len = pks.len();
    for (idx, &col) in cols.iter().enumerate() {
      if cols.iter().take(idx).any(|&x| x == col) { return Err(DupCol(col)); }
      let ci = &*tp.get_ci(col)?;
      if let Some(idx) = pks.iter().position(|&x| x.p() == ci.p()) {
        let p = pks.as_mut_ptr();
        p.add(idx).swap(p.add((new_len -= 1, new_len).1));
      } else { return Err(NoSuchPrimary(col)); }
    }
    if new_len != 0 { check_dup(db, tp, pks.get_unchecked(..new_len))?; }
    for (_, _, ci_id) in db.foreign_links_to(tp_id) {
      let ci = tp.cols.get_unchecked(ci_id as usize);
      if pks.get_unchecked(new_len..).iter().any(|&x| x.p() == ci.p()) &&
        !ci.flags.contains(ColFlags::UNIQUE) { return Err(ForeignOnNotUnique(ci.name())); }
    }
    // now no error can occur
    for &ci in pks.get_unchecked(new_len..) { ci.pr().flags.set(ColFlags::PRIMARY, false); }
    index_unique_primary(db, tp_id, tp);
    Ok(())
  }
}

pub fn add_col<'a>(db: &mut Db, table: &'a str, col: &ColDecl<'a>) -> Result<'a, ()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(table)?;
    if tp.col_num == MAX_COL as u8 { return Err(ColTooMany(tp.col_num as usize + 1)); }
    if col.col.len() > MAX_COL_NAME { return Err(ColNameTooLong(col.col)); }
    if tp.get_ci(col.col).is_ok() { return Err(DupCol(col.col)); }
    let dft = col.dft.unwrap_or(CLit::new(Lit::Null));
    let dft = if !dft.is_null() {
      if col.ty.is_varchar() { return Err(UnsupportedVarcharOp(col.col)); }
      let buf = Align4U8::new(col.ty.size() as usize);
      Some((db.lit2ptr(buf.ptr, col.ty.fix_ty(), dft)?, buf).1)
    } else if col.notnull && tp.count != 0 { return Err(PutNullOnNotNull); } else { None };
    // basically copied from Db::create_table...
    let mut size = ((tp.col_num + 1) as usize + 31) / 32 * 4;
    for ci in tp.cols() { size += ci.ty.size() as usize; }
    size = (size + 3) & !3;
    if size > MAX_DATA_BYTE { return Err(ColSizeTooBig(size)); }
    // now no error can occur
    let bs_size = ((tp.col_num as usize + 31) / 32 * 4, ((tp.col_num + 1) as usize + 31) / 32 * 4);

    let iter = db.record_iter(tp);
    tp.cols.get_unchecked_mut(tp.col_num as usize).init(col.ty, 0, col.col, col.notnull); // `off` will be overwritten in `calc_size`
    tp.col_num += 1;
    calc_size(tp);

    let (size, cap, col_num) = (tp.size as usize, tp.cap, tp.col_num as usize);
    if let Some(dft) = dft.as_ref() {
      let (cp_id, cp) = db.alloc_page::<CheckPage>();
      tp.cols.get_unchecked_mut(col_num - 1).check = (cp_id << 1) | 1;
      cp.count = 0;
      cp.data.as_mut_ptr().copy_from_nonoverlapping(dft.ptr, dft.size);
    }
    let last_off = tp.cols.get_unchecked_mut(col_num - 1).off as usize;
    let (mut dp_id, mut dp) = db.alloc_page::<DataPage>();
    dp.init(!0);
    for (old, _) in iter {
      let new = alloc_slot(db, &mut dp_id, &mut dp, cap, size);
      new.copy_from_nonoverlapping(old, bs_size.0);
      new.add(bs_size.1).copy_from_nonoverlapping(old.add(bs_size.0), last_off - bs_size.1);
      if let Some(dft) = dft.as_ref() {
        bsdel(new as *mut u32, col_num - 1);
        new.add(last_off).copy_from_nonoverlapping(dft.ptr, dft.size);
      } else { bsset(new as *mut u32, col_num - 1); }
    }
    reset_data(db, tp_id, tp, dp_id, dp);
    index_unique_primary(db, tp_id, tp); // it is currently useless, because `add_col` won't affect primary keys
    Ok(())
  }
}

pub fn drop_col<'a>(db: &mut Db, table: &'a str, col: &'a str) -> Result<'a, ()> {
  unsafe {
    let (tp_id, tp) = db.get_tp(table)?;
    let col_num = tp.col_num as usize;
    let ci = tp.get_ci(col)?;
    let ci_id = ci.idx(&tp.cols) as usize;
    if col_num == 1 { return Err(ColTooFew); }
    if db.foreign_links_to(tp_id).any(|x| x.2 == ci_id as u8) { return Err(ModifyTableWithForeignLink(table)); }
    if ci.flags.contains(ColFlags::PRIMARY) {
      let pks = tp.primary_cols().filter(|&x| x.p() != ci.p()).collect::<Vec<_>>();
      if !pks.is_empty() { check_dup(db, tp, &pks)?; }
    }
    // now no error can occur
    let bs_size = ((col_num + 31) / 32 * 4, (col_num - 1 + 31) / 32 * 4);
    let l_size = ci.off as usize - bs_size.0;
    // the padding in right side may change, so need to copy data one by one; r_size_off is Vec<(size, old off, new off)>
    let mut r_size_off = tp.cols.get_unchecked(ci_id + 1..col_num).iter().map(|ci| (ci.ty.size(), ci.off, 0u16)).collect::<Vec<_>>();

    if ci.index != !0 { db.dealloc_index(ci.index); }
    if ci.check != !0 { db.dealloc_page(ci.check >> 1); }
    if ci.ty.is_varchar() {
      for (data, _) in db.record_iter(tp) {
        if !is_null(data, ci_id as u32) { db.free_varchar(data.add(ci.off as usize)); }
      }
    }

    let iter = db.record_iter(tp); // it will iterate over old data because necessary information is copied into iter
    tp.cols.as_mut_ptr().add(ci_id).copy_from(tp.cols.as_mut_ptr().add(ci_id + 1), col_num - ci_id - 1);
    tp.col_num -= 1;
    calc_size(tp);

    let (size, cap, col_num) = (tp.size as usize, tp.cap, tp.col_num as usize);
    for idx in ci_id..col_num {
      r_size_off.get_unchecked_mut(idx - ci_id).2 = tp.cols.get_unchecked(idx).off;
    }
    let (mut dp_id, mut dp) = db.alloc_page::<DataPage>();
    dp.init(!0);
    for (old, _) in iter {
      let new = alloc_slot(db, &mut dp_id, &mut dp, cap, size);
      (new as *mut u32).write_bytes(0, bs_size.1);
      for i in 0..ci_id {
        if is_null(old, i as u32) { bsset(new as *mut u32, i); }
      }
      for i in ci_id + 1..col_num {
        if is_null(old, i as u32) { bsset(new as *mut u32, i - 1); }
      }
      new.add(bs_size.1).copy_from_nonoverlapping(old.add(bs_size.0), l_size);
      for &(size, old_off, new_off) in &r_size_off {
        new.add(new_off as usize).copy_from_nonoverlapping(old.add(old_off as usize), size as usize);
      }
    }
    reset_data(db, tp_id, tp, dp_id, dp);
    index_unique_primary(db, tp_id, tp);
    Ok(())
  }
}

unsafe fn calc_size(tp: &mut TablePage) {
  let mut size = (tp.col_num as u16 + 31) / 32 * 4;
  for ci in tp.cols() {
    if ci.ty.align4() { size = (size + 3) & !3; }
    ci.pr().off = size;
    size += ci.ty.size();
  }
  size = (size + 3) & !3;
  (tp.size = size, tp.cap = MAX_DATA_BYTE as u16 / size);
}

unsafe fn alloc_slot(db: &mut Db, dp_id: &mut u32, dp: &mut &mut DataPage, cap: u16, size: usize) -> *mut u8 {
  if dp.count == cap {
    let (new_dp_id, new_dp) = db.alloc_page::<DataPage>();
    new_dp.init(*dp_id);
    (*dp_id = new_dp_id, *dp = new_dp);
  }
  let cur = (dp.count as usize, dp.count += 1).0;
  bsset(dp.used.as_mut_ptr(), cur);
  dp.data.as_mut_ptr().add(cur * size)
}

unsafe fn reset_data(db: &mut Db, tp_id: u32, tp: &mut TablePage, dp_id: u32, dp: &DataPage) {
  db.drop_list(tp.first);
  tp.first = dp_id;
  tp.first_free = if dp.count == tp.cap { !0 } else { dp_id };
  for ci in tp.cols() {
    if ci.index != !0 {
      db.dealloc_index(ci.index);
      let (id, ip) = db.alloc_page::<IndexPage>();
      ci.pr().index = id;
      ip.init(true, ci.ty.size());
      insert_all(db, tp_id, tp, ci);
    }
  }
}


unsafe fn index_unique_primary(db: &mut Db, tp_id: u32, tp: &TablePage) {
  for (idx, ci) in tp.cols().iter().enumerate() {
    if ci.flags.contains(ColFlags::PRIMARY) {
      if ci.index == !0 && !tp.cols().get_unchecked(idx + 1..).iter().any(|ci| ci.flags.contains(ColFlags::PRIMARY)) {
        db.alloc_index(ci.pr(), "").unchecked_unwrap();
        insert_all(db, tp_id, tp, ci);
      }
      break;
    }
  }
}

unsafe fn insert_all(db: &mut Db, tp_id: u32, tp: &TablePage, ci: &ColInfo) {
  let ci_id = ci.idx(&tp.cols);
  macro_rules! handle {
    ($ty: ident) => {{
      let mut index = Index::<{ $ty }>::new(db, tp_id, ci_id);
      for (data, rid) in db.record_iter(tp) {
        if !is_null(data, ci_id) { index.insert(data.add(ci.off as usize), rid); }
      }
    }};
  }
  handle_all!(ci.ty.fix_ty().ty, handle);
}

unsafe fn check_dup<'a>(db: &mut Db, tp: &TablePage, pks: &[&ColInfo]) -> Result<'a, ()> {
  let mut pk_set = HashSet::default();
  for (data, _) in db.record_iter(tp) {
    if !pk_set.insert(hash_pks(data, &pks)) { return Err(PutDupOnPrimary); }
  }
  Ok(())
}