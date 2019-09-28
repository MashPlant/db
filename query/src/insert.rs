use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use physics::*;
use index::{Index, cmp::Cmp};
use db::{Db, fill_ptr};
use crate::{is_null, handle_all};

struct InsertCtx<'a> {
  tp_id: usize,
  tp: &'a mut TablePage,
  pks: Vec<&'a ColInfo>,
  pk_set: HashSet<u64>,
}

impl InsertCtx<'_> {
  unsafe fn build<'a>(db: &mut Db, table: &str) -> Result<InsertCtx<'a>> {
    let (tp_id, tp) = db.get_tp(table)?;
    let pks = tp.cols().iter().filter(|ci| ci.flags.contains(ColFlags::PRIMARY)).collect::<Vec<_>>();
    let pk_set: HashSet<_> = if pks.len() > 1 {
      db.record_iter((tp_id, tp)).map(|(data, _)| Self::hash_pks(data.as_ptr(), &pks)).collect()
    } else { HashSet::new() }; // no need to collect
    Ok(InsertCtx { tp, tp_id, pks, pk_set })
  }

  unsafe fn work(i: &Insert, db: &mut Db) -> Result<()> {
    let mut ctx = InsertCtx::build(db, i.table)?;
    let slot_size = ctx.tp.size as usize;
    let buf = Align4U8::new(slot_size);
    for vals in &i.vals {
      ctx.fill_buf(buf.ptr, vals)?;
      ctx.insert_ck(buf.ptr, vals, db)?;
      // now it can't fail, do insert
      let rid = db.allocate_data_slot(ctx.tp_id); // the `used` bit is set here, and `count` grows here
      let (page, slot) = (rid.page(), rid.slot());
      debug_assert!(slot < ctx.tp.cap as u32);
      let dp = db.get_page::<DataPage>(page as usize);
      dp.data.as_mut_ptr().add(slot as usize * slot_size).copy_from_nonoverlapping(buf.ptr, slot_size);
      // update index
      for i in 0..vals.len() {
        let col = ctx.tp.cols.get_unchecked(i);
        if col.index != !0 && !is_null(buf.ptr, i) {  // null item doesn't get inserted to index
          let ptr = buf.ptr.add(col.off as usize);
          macro_rules! handle {
            ($ty: ident) => {{
              let mut index = Index::<{ $ty }>::new(db, Rid::new(ctx.tp_id as u32, i as u32));
              index.insert(ptr, rid);
            }};
          }
          handle_all!(col.ty.ty, handle);
        }
      }
    }
    Ok(())
  }

  unsafe fn fill_buf(&self, buf: *mut u8, vals: &Vec<Lit>) -> Result<()> {
    let tp = &*self.tp;
    if vals.len() != tp.col_num as usize { return Err(InsertLenMismatch { expect: tp.col_num, actual: vals.len() }); }
    buf.write_bytes(0, (vals.len() + 31) / 32); // clear null-bitset
    for (idx, &val) in vals.iter().enumerate() {
      let ci = tp.cols.get_unchecked(idx);
      if val.is_null() {
        // primary implies notnull, so inserting null to primary key will be rejected here
        if ci.flags.contains(ColFlags::NOTNULL) { return Err(PutNullOnNotNull); }
        *(buf as *mut u32).add(idx / 32) |= 1 << ((idx % 32) as u32);
      } else {
        fill_ptr(buf.add(ci.off as usize), ci.ty, val)?;
      }
    }
    Ok(())
  }

  unsafe fn insert_ck(&mut self, data: *const u8, vals: &Vec<Lit>, db: &mut Db) -> Result<()> {
    debug_assert_eq!(vals.len(), self.tp.col_num as usize); // fill_buf guarantees this
    'out: for i in 0..vals.len() {
      // below are unique / foreign / `check` check, null item doesn't need them (null check is in `fill_buf`)
      if !is_null(data, i) {
        let ci = self.tp.cols.get_unchecked(i);
        let ptr = data.add(ci.off as usize);
        if ci.flags.contains(ColFlags::UNIQUE) { // all unique keys have index
          debug_assert_ne!(ci.index, !0);
          macro_rules! handle {
            ($ty: ident) => {{
              let index = Index::<{ $ty }>::new(db, Rid::new(self.tp_id as u32, i as u32));
              if index.contains(ptr) {
                return Err(InsertDupOnUniqueKey { col: ci.name().into(), val: vals.get_unchecked(i).to_owned() });
              }
            }};
          }
          handle_all!(ci.ty.ty, handle);
        }
        if ci.foreign_table != !0 {
          let dp = db.get_page::<DbPage>(0);
          debug_assert!(ci.foreign_table < dp.table_num);
          let f_table_page = db.get_page::<DbPage>(0).tables.get_unchecked(ci.foreign_table as usize).meta;
          let f_tp = db.get_page::<TablePage>(f_table_page as usize);
          debug_assert!(ci.foreign_col < f_tp.col_num);
          debug_assert!(f_tp.cols.get_unchecked(ci.foreign_col as usize).index != !0);
          macro_rules! handle {
            ($ty: ident) => {{
              let index = Index::<{ $ty }>::new(db, Rid::new(f_table_page, ci.foreign_col as u32));
              if !index.contains(ptr) {
                return Err(InsertNoExistOnForeignKey { col: ci.name().into(), val: vals.get_unchecked(i).to_owned() });
              }
            }};
          }
          // in `db.rs` we already guarantee their types are compatible, and if they are both string, the inserted one must be longer
          handle_all!(ci.ty.ty, handle);
        }
        if ci.check != !0 {
          let cp = db.get_page::<CheckPage>(ci.check as usize);
          let sz = ci.ty.size() as usize;
          let mut off = 0;
          macro_rules! handle {
            ($ty: ident) => {{
              if Cmp::<{ $ty }>::cmp(ptr, cp.data.as_ptr().add(off)) == std::cmp::Ordering::Equal {
                continue 'out;
              }
            }};
          }
          for _ in 0..cp.len {
            debug_assert!(off + sz < MAX_CHECK_BYTES);
            handle_all!(ci.ty.ty, handle);
            off += sz;
          }
          // the `continue` in macro can prevent it
          return Err(InsertNotInCheck { col: ci.name().into(), val: vals.get_unchecked(i).to_owned() });
        }
      }
    }
    if self.pks.len() > 1 {
      let hash = Self::hash_pks(data, &self.pks);
      if !self.pk_set.insert(hash) { // existed
        return Err(InsertDupCompositePrimaryKey);
      }
    }
    Ok(())
  }

  unsafe fn hash_pks(data: *const u8, pks: &Vec<&ColInfo>) -> u64 {
    const SEED: u64 = 19260817;
    let mut hash = 0u64;
    for &col in pks {
      let ptr = data.add(col.off as usize);
      match col.ty {
        ColTy { ty: Char, size } | ColTy { ty: VarChar, size } => for i in 0..size as usize {
          hash = hash.wrapping_mul(SEED).wrapping_add(*ptr.add(i) as u64);
        }
        ColTy { ty: Int, .. } | ColTy { ty: Float, .. } | ColTy { ty: Date, .. } =>
          hash = hash.wrapping_mul(SEED).wrapping_add(*(ptr as *const u32) as u64),
        ColTy { ty: Bool, .. } => hash = hash.wrapping_mul(SEED).wrapping_add(*ptr as u64),
      }
    }
    hash
  }
}

pub fn insert(i: &Insert, db: &mut Db) -> Result<()> { unsafe { InsertCtx::work(i, db) } }