use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use physics::*;
use index::Index;
use db::Db;
use crate::{is_null, fill_ptr, handle_all};

struct InsertCtx<'a> {
  tp: &'a mut TablePage,
  table_page: usize,
  pks: Vec<&'a ColInfo>,
  pk_set: HashSet<u64>,
}

impl InsertCtx<'_> {
  unsafe fn build<'a>(db: &mut Db, table_page: usize) -> InsertCtx<'a> {
    let tp = db.get_page::<TablePage>(table_page);
    let mut pks = Vec::new();
    for i in 0..tp.col_num as usize {
      if tp.cols.get_unchecked(i).flags.contains(ColFlags::PRIMARY) {
        pks.push(tp.pr().cols.get_unchecked(i));
      }
    }
    let pk_set: HashSet<_> = if pks.len() > 1 {
      db.record_iter(tp).map(|(data, _)| Self::hash_pks(data.as_ptr(), &pks)).collect()
    } else { HashSet::new() }; // no need to collect
    InsertCtx { tp, table_page, pks, pk_set }
  }

  unsafe fn work(i: &Insert, db: &mut Db) -> Result<()> {
    let ti = db.get_ti(i.table)?;
    let mut ctx = InsertCtx::build(db, ti.meta as usize);
    let slot_size = ctx.tp.size as usize;
    let buf = Align4U8::new(slot_size);
    for vals in &i.vals {
      ctx.fill_buf(buf.ptr, vals)?;
      ctx.insert_ck(buf.ptr, vals, db)?;
      // now it can't fail, do insert
      let rid = db.allocate_data_slot(ctx.tp); // the `used` bit is set here, and `count` grows here
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
              let mut index = Index::<{ $ty }>::new(db, Rid::new(ctx.table_page as u32, i as u32));
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
        // primary implies notnull, so inserting null to primary key  will be rejected here
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
    for i in 0..vals.len() {
      let col = self.tp.cols.get_unchecked(i);
      let mut ptr = data.add(col.off as usize);
      if col.flags.contains(ColFlags::UNIQUE) { // all unique keys have index
        debug_assert_ne!(col.index, !0);
        if !is_null(data, i) {
          // null item doesn't need uniqueness check
          macro_rules! handle {
            ($ty: ident) => {{
              let index = Index::<{ $ty }>::new(db, Rid::new(self.table_page as u32, i as u32));
              if index.contains(ptr) {
                return Err(InsertDupOnUniqueKey { col: col.name().into(), val: vals.get_unchecked(i).to_owned() });
              }
            }};
          }
          handle_all!(col.ty.ty, handle);
        }
      }
      if col.foreign_table != !0 {
        let dp = db.get_page::<DbPage>(0);
        debug_assert!(col.foreign_table < dp.table_num);
        let f_table_page = db.get_page::<DbPage>(0).tables.get_unchecked(col.foreign_table as usize).meta;
        let f_tp = db.get_page::<TablePage>(f_table_page as usize);
        debug_assert!(col.foreign_col < f_tp.col_num);
        let f_col = f_tp.cols.get_unchecked(col.foreign_col as usize);
        macro_rules! handle {
          ($ty: ident) => {{
            let index = Index::<{ $ty }>::new(db, Rid::new(f_table_page, col.foreign_col as u32));
            if !index.contains(ptr) {
              return Err(InsertNoExistOnForeignKey { col: col.name().into(), val: vals.get_unchecked(i).to_owned() });
            }
          }};
        }
        match col.ty.ty {
          Int => handle!(Int), Bool => handle!(Bool), Float => handle!(Float), Date => handle!(Date),
          // the string in this col and foreign col may have different length
          // since Index will access as much memory as foreign col's string, if it is longer than this col's
          // it may cause access violation
          Char | VarChar => if col.ty.size >= f_col.ty.size { // this col's string is not shorter, no need of special treatment
            handle!(Char) // actually Char and VarChar are handled in the same way
          } else {
            let tmp = Align4U8::new(f_col.ty.size as usize + 1);
            tmp.ptr.copy_from_nonoverlapping(ptr, col.ty.size as usize + 1);
            ptr = tmp.ptr; // `handle` use this `ptr` instead of the previous one
            handle!(Char)
          }
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

#[inline(always)]
pub fn insert(i: &Insert, db: &mut Db) -> Result<()> { unsafe { InsertCtx::work(i, db) } }