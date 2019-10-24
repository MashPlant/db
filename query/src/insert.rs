use unchecked_unwrap::UncheckedUnwrap;
use std::{borrow::Cow::{self, *}, cmp::Ordering::*};

use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use physics::*;
use index::{Index, cmp::Cmp, handle_all};
use db::{Db, is_null, fill_ptr, ptr2lit, hash_pks};

// update can also use this
pub(crate) struct InsertCtx<'a> {
  db: &'a mut Db,
  pub(crate) tp_id: u32,
  pub(crate) tp: &'a mut TablePage,
  pub(crate) pks: Vec<&'a ColInfo>,
  pub(crate) pk_set: HashSet<u128>,
  // these 2 not used in update (it may be a little waste, but is acceptable)
  cols: Option<Box<[u32]>>,
  dfts: Box<[CLit<'a>]>,
}

impl<'a> InsertCtx<'a> {
  pub(crate) unsafe fn new<'b>(db: &mut Db, table: &'b str, cols: Option<&[&'b str]>) -> Result<'b, InsertCtx<'a>> {
    let (tp_id, tp) = db.get_tp(table)?;
    let pks = tp.primary_cols().collect::<Vec<_>>();
    let pk_set: HashSet<_> = if pks.len() > 1 {
      db.record_iter(tp).map(|(data, _)| hash_pks(data, &pks)).collect()
    } else { HashSet::new() }; // no need to collect
    let cols = if let Some(cols1) = cols {
      let mut cols = vec![0; cols1.len()].into_boxed_slice();
      for (idx, c) in cols1.iter().enumerate() {
        *cols.get_unchecked_mut(idx) = tp.get_ci(c)?.idx(&tp.cols);
      }
      Some(cols)
    } else { None };
    let mut dfts = vec![CLit::new(Lit::Null); tp.col_num as usize].into_boxed_slice();
    for (idx, ci) in tp.cols().iter().enumerate() {
      if ci.check != !0 && ((ci.check & 1) == 1) {
        let cp = db.get_page::<CheckPage>(ci.check >> 1);
        let ptr = cp.data.as_ptr().add(cp.count as usize * ci.ty.size() as usize); // the one-past-last slot
        *dfts.get_unchecked_mut(idx) = ptr2lit(ptr, ci.ty.ty);
      }
    }
    Ok(InsertCtx { db: db.pr(), tp, tp_id, pks, pk_set, cols, dfts })
  }

  // result's len == table's col num
  unsafe fn get_insert_val<'b, 'c>(&self, vals: &'c [CLit<'a>]) -> Result<'b, Cow<'c, [CLit<'a>]>> {
    if let Some(cols) = &self.cols {
      if cols.len() < vals.len() { return Err(InsertTooLong { max: cols.len(), actual: vals.len() }); }
      let mut ret = self.dfts.to_vec();
      for (&v, &c) in vals.iter().zip(cols.iter()) {
        *ret.get_unchecked_mut(c as usize) = v;
      }
      Ok(Cow::Owned(ret))
    } else {
      match vals.len().cmp(&(self.tp.col_num as usize)) {
        Less => {
          let mut ret = self.dfts.to_vec();
          ret.as_mut_ptr().copy_from_nonoverlapping(vals.as_ptr(), vals.len());
          Ok(Owned(ret))
        }
        Equal => Ok(Cow::Borrowed(vals)),
        Greater => Err(InsertTooLong { max: self.tp.col_num as usize, actual: vals.len() })
      }
    }
  }

  unsafe fn insert(&mut self, buf: *mut u8, vals: &[CLit<'a>]) -> Result<'a, ()> {
    let vals = self.get_insert_val(vals)?;
    (buf as *mut u32).write_bytes(0, (vals.len() + 31) / 32); // clear null-bitset
    for (idx, &val) in vals.iter().enumerate() {
      let ci = self.tp.cols.get_unchecked(idx);
      if val.is_null() {
        if ci.flags.intersects(ColFlags::NOTNULL1) { return Err(PutNullOnNotNull); }
        bsset(buf as *mut u32, idx);
      } else {
        fill_ptr(buf.add(ci.off as usize), ci.ty, val)?;
      }
    }
    for ci_id in 0..self.tp.col_num as u32 {
      self.check_col(buf, ci_id, *vals.get_unchecked(ci_id as usize), None)?;
    }
    if self.pks.len() > 1 {
      if !self.pk_set.insert(hash_pks(buf, &self.pks)) { return Err(PutDupOnPrimary); }
    }
    // now no error can occur
    self.tp.count += 1;
    let rid = self.db.allocate_data_slot(self.tp_id); // the `used` bit is set here, and `count` grows here
    let (page, slot) = (rid.page(), rid.slot());
    let dp = self.db.get_page::<DataPage>(page);
    let size = self.tp.size as usize;
    dp.data.as_mut_ptr().add(slot as usize * size).copy_from_nonoverlapping(buf, size);
    // update index
    for (ci_id, ci) in self.tp.cols().iter().enumerate() {
      if ci.index != !0 && !is_null(buf, ci_id as u32) {  // null item doesn't get inserted to index
        let ptr = buf.add(ci.off as usize);
        macro_rules! handle {
          ($ty: ident) => {{ Index::<{ $ty }>::new(self.db, self.tp_id, ci_id as u32).insert(ptr, rid); }};
        }
        handle_all!(ci.ty.ty, handle);
      }
    }
    Ok(())
  }

  // `rid` is used for unique check, if rid is Some && a rid `rid1` is found in Index && `rid1` is equal to `rid`, it is not regarded as a duplicate
  // the return value's life time can't come from `data`, because `data` are on the stack in all usage
  pub(crate) unsafe fn check_col(&mut self, data: *const u8, ci_id: u32, val: CLit<'a>, rid: Option<Rid>) -> Result<'a, ()> {
    // unique / foreign / `check` check, null item doesn't need them (null check is in `fill_buf`)
    if !is_null(data, ci_id) {
      let ci = self.tp.cols.get_unchecked(ci_id as usize);
      let ptr = data.add(ci.off as usize);
      if ci.unique(self.pks.len()) {
        macro_rules! handle {
          ($ty: ident) => {{
            let mut index = Index::<{ $ty }>::new(self.db, self.tp_id, ci_id);
            let (mut it, end) = (index.lower_bound(ptr), index.upper_bound(ptr));
            while it != end {
              if rid != Some(it.next().unchecked_unwrap()) { return Err(PutDupOnUnique { col: ci.name(), val }); }
            }
          }};
        }
        handle_all!(ci.ty.ty, handle);
      }
      if ci.f_table != !0 {
        macro_rules! handle {
          ($ty: ident) => {{
            if !Index::<{ $ty }>::new(self.db, ci.f_table, ci.f_col as u32).contains(ptr) { return Err(PutNonexistentForeign { col: ci.name(), val }); }
          }};
        }
        handle_all!(ci.ty.ty, handle); // their type are exactly the same, so can use `ptr` directly to search in index, `create_table` guarantee this
      }
      if ci.check != !0 {
        let cp = self.db.get_page::<CheckPage>(ci.check >> 1);
        if cp.count == 0 { return Ok(()); } // the check page only contains default value (actually no check)
        let sz = ci.ty.size() as usize;
        macro_rules! handle {
          ($ty: ident) => {{
            for i in 0..cp.count as usize {
              if Cmp::<{ $ty }>::cmp(ptr, cp.data.as_ptr().add(i * sz)) == Equal { return Ok(()); }
            }
          }};
        }
        handle_all!(ci.ty.ty, handle);
        // the `return` in macro can prevent it
        return Err(PutNotInCheck { col: ci.name(), val });
      }
    }
    Ok(())
  }
}

pub fn insert<'a>(i: &Insert<'a>, db: &mut Db) -> ModifyResult<'a, u32> {
  unsafe {
    let mut ctx = InsertCtx::new(db, i.table, i.cols.as_deref())?;
    let buf = Align4U8::new(ctx.tp.size as usize);
    let mut cnt = 0;
    for vals in &i.vals {
      if let Err(e) = ctx.insert(buf.ptr, vals) { return Err(ModifyError(cnt, e)); }
      cnt += 1;
    }
    Ok(cnt)
  }
}