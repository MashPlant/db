use unchecked_unwrap::UncheckedUnwrap;

use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use physics::*;
use index::{Index, cmp::Cmp, handle_all};
use db::{Db, fill_ptr};
use crate::is_null;

// update can also use this
pub(crate) struct InsertCtx<'a> {
  pub(crate) db: &'a mut Db,
  pub(crate) tp_id: u32,
  pub(crate) tp: &'a mut TablePage,
  pub(crate) pks: Vec<&'a ColInfo>,
  pub(crate) pk_set: HashSet<u64>,
}

impl InsertCtx<'_> {
  pub(crate) unsafe fn build<'a, 'b>(db: &mut Db, table: &'b str) -> Result<'b, InsertCtx<'a>> {
    let (tp_id, tp) = db.get_tp(table)?;
    let pks = tp.cols().iter().filter(|ci| ci.flags.contains(ColFlags::PRIMARY)).collect::<Vec<_>>();
    let pk_set: HashSet<_> = if pks.len() > 1 {
      db.record_iter(tp_id, tp).map(|(data, _)| Self::hash_pks(data.as_ptr(), &pks)).collect()
    } else { HashSet::new() }; // no need to collect
    Ok(InsertCtx { db: db.pr(), tp, tp_id, pks, pk_set })
  }

  unsafe fn fill_buf<'a>(&self, buf: *mut u8, vals: &Vec<CLit<'a>>) -> Result<'a, ()> {
    let tp = &*self.tp;
    if vals.len() != tp.col_num as usize { return Err(InsertLenMismatch { expect: tp.col_num, actual: vals.len() }); }
    (buf as *mut u32).write_bytes(0, (vals.len() + 31) / 32); // clear null-bitset
    for (idx, &val) in vals.iter().enumerate() {
      let ci = tp.cols.get_unchecked(idx);
      if val.is_null() {
        // primary implies notnull, so inserting null to primary key will be rejected here
        if ci.flags.contains(ColFlags::NOTNULL) { return Err(PutNullOnNotNull); }
        bsset(buf as *mut u32, idx);
      } else {
        fill_ptr(buf.add(ci.off as usize), ci.ty, val)?;
      }
    }
    Ok(())
  }

  // `rid` is used for unique check, if rid is Some && a rid `rid1` is found in Index && `rid1` is equal to `rid`, it is not regarded as a duplicate
  pub(crate) unsafe fn check_col<'a>(&mut self, data: *const u8, ci_id: u32, val: CLit<'a>, rid: Option<Rid>) -> Result<'a, ()> {
    debug_assert!((ci_id as usize) < self.tp.cols.len());
    //  unique / foreign / `check` check, null item doesn't need them (null check is in `fill_buf`)
    if !is_null(data, ci_id) {
      let ci = self.tp.cols.get_unchecked(ci_id as usize);
      let ptr = data.add(ci.off as usize);
      if ci.flags.contains(ColFlags::UNIQUE) {
        debug_assert_ne!(ci.index, !0); // all unique keys have index, `create_table` guarantee this
        macro_rules! handle {
          ($ty: ident) => {{
            let mut index = Index::<{ $ty }>::new(self.db, self.tp_id, ci_id);
            let (mut it, end) = (index.lower_bound(ptr), index.upper_bound(ptr));
            while it != end {
              let rid1 = it.next().unchecked_unwrap();
              if Some(rid1) != rid { return Err(PutDupOnUniqueKey { col: ci.name(), val }); }
            }
          }};
        }
        handle_all!(ci.ty.ty, handle);
      }
      if ci.foreign_table != !0 {
        let f_tp = self.db.get_page::<TablePage>(ci.foreign_table);
        debug_assert!(ci.foreign_col < f_tp.col_num);
        debug_assert!(f_tp.cols.get_unchecked(ci.foreign_col as usize).index != !0);
        macro_rules! handle {
          ($ty: ident) => {{
            let index = Index::<{ $ty }>::new(self.db, ci.foreign_table, ci.foreign_col as u32);
            if !index.contains(ptr) {
              return Err(PutNonexistentForeignKey { col: ci.name(), val });
            }
          }};
        }
        // in `db.rs` we already guarantee their types are compatible, and if they are both string, the inserted one must be longer
        handle_all!(ci.ty.ty, handle);
      }
      if ci.check != !0 {
        let cp = self.db.get_page::<CheckPage>(ci.check);
        let sz = ci.ty.size() as usize;
        let mut off = 0;
        macro_rules! handle {
          ($ty: ident) => {{
            if Cmp::<{ $ty }>::cmp(ptr, cp.data.as_ptr().add(off)) == std::cmp::Ordering::Equal {
              return Ok(());
            }
          }};
        }
        for _ in 0..cp.len {
          debug_assert!(off + sz < MAX_CHECK_BYTES);
          handle_all!(ci.ty.ty, handle);
          off += sz;
        }
        // the `return` in macro can prevent it
        return Err(PutNotInCheck { col: ci.name(), val });
      }
    }
    Ok(())
  }

  pub unsafe fn hash_pks(data: *const u8, pks: &Vec<&ColInfo>) -> u64 {
    const SEED: u64 = 19260817;
    let mut hash = 0u64;
    for &col in pks {
      let ptr = data.add(col.off as usize);
      match col.ty.ty {
        Bool => hash = hash.wrapping_mul(SEED).wrapping_add(*ptr as u64),
        Int | Float | Date => hash = hash.wrapping_mul(SEED).wrapping_add(*(ptr as *const u32) as u64),
        VarChar => for &b in str_from_db(ptr).as_bytes() {
          hash = hash.wrapping_mul(SEED).wrapping_add(b as u64);
        }
      }
    }
    hash
  }
}

pub fn insert<'a>(i: &Insert<'a>, db: &mut Db) -> Result<'a, ()> {
  unsafe {
    let mut ctx = InsertCtx::build(db, i.table)?;
    let slot_size = ctx.tp.size as usize;
    let buf = Align4U8::new(slot_size);
    for vals in &i.vals {
      ctx.fill_buf(buf.ptr, vals)?;
      for (idx, &val) in vals.iter().enumerate() {
        ctx.check_col(buf.ptr, idx as u32, val, None)?;
      }
      if ctx.pks.len() > 1 {
        if !ctx.pk_set.insert(InsertCtx::hash_pks(buf.ptr, &ctx.pks)) { return Err(PutDupCompositePrimaryKey); }
      }
      // now it can't fail, do insertion
      ctx.tp.count += 1;
      let rid = db.allocate_data_slot(ctx.tp_id); // the `used` bit is set here, and `count` grows here
      let (page, slot) = (rid.page(), rid.slot());
      debug_assert!(slot < ctx.tp.cap as u32);
      let dp = db.get_page::<DataPage>(page);
      dp.data.as_mut_ptr().add(slot as usize * slot_size).copy_from_nonoverlapping(buf.ptr, slot_size);
      // update index
      for i in 0..vals.len() {
        let ci = ctx.tp.cols.get_unchecked(i);
        if ci.index != !0 && !is_null(buf.ptr, i as u32) {  // null item doesn't get inserted to index
          let ptr = buf.ptr.add(ci.off as usize);
          macro_rules! handle {
            ($ty: ident) => {{
              let mut index = Index::<{ $ty }>::new(db, ctx.tp_id, i as u32);
              index.insert(ptr, rid);
            }};
          }
          handle_all!(ci.ty.ty, handle);
        }
      }
    }
    Ok(())
  }
}