use std::{fs::{File, OpenOptions}, path::Path};
use memmap::{MmapOptions, MmapMut};

use physics::*;
use common::{*, Error::*, BareTy::*};
use syntax::ast::*;
use unchecked_unwrap::UncheckedUnwrap;
use crate::fill_ptr;

pub struct Db {
  pub(crate) mmap: MmapMut,
  pub(crate) pages: usize,
  pub(crate) file: File,
  pub(crate) path: String,
}

impl Db {
  pub fn create(path: impl AsRef<Path>) -> Result<Db> {
    unsafe {
      let file = OpenOptions::new().read(true).write(true).create(true).append(true).open(path.as_ref())?;
      file.set_len(PAGE_SIZE as u64)?;
      // this is 64G, the maximum capacity of this db; mmap will not allocate memory unless accessed
      let mut mmap = MmapOptions::new().len(PAGE_SIZE * MAX_PAGE).map_mut(&file)?;
      (mmap.as_mut_ptr() as *mut DbPage).r().init();
      Ok(Db { mmap, pages: 1, file, path: path.as_ref().to_string_lossy().into_owned() })
    }
  }

  pub fn open(path: impl AsRef<Path>) -> Result<Db> {
    unsafe {
      let file = OpenOptions::new().read(true).write(true).append(true).open(path.as_ref())?;
      let size = file.metadata()?.len() as usize;
      if size == 0 || size % PAGE_SIZE != 0 { return Err(InvalidSize(size)); }
      let mut mmap = MmapOptions::new().len(PAGE_SIZE * MAX_PAGE).map_mut(&file)?;
      let dp = (mmap.as_mut_ptr() as *mut DbPage).r();
      if &dp.magic != MAGIC {
        return Err(InvalidMagic(dp.magic));
      }
      Ok(Db { mmap, pages: size / PAGE_SIZE, file, path: path.as_ref().to_string_lossy().into_owned() })
    }
  }

  pub fn path(&self) -> &str { &self.path }
}

impl Db {
  pub fn create_table(&mut self, c: &CreateTable) -> Result<()> {
    unsafe {
      let dp = self.get_page::<DbPage>(0);

      // validate table
      if dp.table_num == MAX_TABLE as u8 { return Err(TableExhausted); }
      if c.name.len() as u32 >= MAX_TABLE_NAME { return Err(TableNameTooLong(c.name.into())); }
      if dp.names().find(|&name| name == c.name).is_some() { return Err(DupTable(c.name.into())); }
      if c.cols.len() >= MAX_COL as usize { return Err(ColTooMany(c.cols.len())); }
      let mut cols = IndexSet::default();
      for co in &c.cols {
        if cols.contains(co.name) { return Err(DupCol(co.name.into())); }
        if co.name.len() as u32 >= MAX_COL_NAME { return Err(ColNameTooLong(co.name.into())); }
        cols.insert(co.name);
      }

      // validate col cons
      let mut primary_cnt = 0;
      // only allow one one primary / foreign / check for one col
      let mut has_pfc = vec![(false, false, false); c.cons.len()];

      for cons in &c.cons {
        if let Some((idx, _)) = cols.get_full(cons.name) {
          match &cons.kind {
            TableConsKind::Primary => {
              if has_pfc[idx].0 { return Err(DupPrimary(cons.name.into())); }
              has_pfc[idx].0 = true;
              primary_cnt += 1;
            }
            &TableConsKind::Foreign { table, col } => {
              if has_pfc[idx].1 { return Err(DupForeign(cons.name.into())); }
              has_pfc[idx].1 = true;
              let ci = self.get_tp(table)?.1.get_ci(col)?.1;
              if !ci.flags.contains(ColFlags::UNIQUE) { return Err(ForeignKeyOnNonUnique(col.into())); }
              let (f_ty, ty) = (ci.ty, c.cols[idx].ty);
              // strict here, don't allow foreign link between two types or shorter string to longer string
              // (for simplicity of future handling)
              match (f_ty.ty, ty.ty) {
                (Char, Char) | (Char, VarChar) | (VarChar, Char) | (VarChar, VarChar) if ty.size >= f_ty.size => {}
                (Int, Int) | (Bool, Bool) | (Float, Float) | (Date, Date) => {}
                _ => return Err(IncompatibleForeignTy { foreign: f_ty, own: ty }),
              }
            }
            TableConsKind::Check(check) => {
              if has_pfc[idx].2 { return Err(DupCheck(cons.name.into())); }
              has_pfc[idx].2 = true;
              let ty = c.cols[idx].ty;
              let sz = ty.size() as usize;
              if check.len() * sz >= MAX_CHECK_BYTES { return Err(CheckTooLong(cons.name.into(), check.len())); }
              let buf = Align4U8::new(ty.size() as usize); // dummy buffer
              for &c in check {
                match c {
                  Lit::Null => return Err(CheckNull(cons.name.into())),
                  _ => fill_ptr(buf.ptr, ty, c)?,
                }
              }
            }
          }
        } else { return Err(NoSuchCol(cons.name.into())); }
      }

      // validate size, the size is calculated in the same way as below
      let mut size = (c.cols.len() as u16 + 31) / 32 * 4; // null bitset
      for c in &c.cols {
        size += c.ty.size();
        if size as usize > MAX_DATA_BYTE { return Err(ColSizeTooBig(size as usize)); }
      }
      size = (size + 3) & !3; // at last it should be aligned to keep the alignment of the next slot
      if size as usize > MAX_DATA_BYTE { return Err(ColSizeTooBig(size as usize)); }

      // now no error can occur, can write to db safely

      // handle each col def
      let (id, tp) = self.allocate_page::<TablePage>();
      let mut size = (c.cols.len() as u16 + 31) / 32 * 4; // null bitset
      for (i, c) in c.cols.iter().enumerate() {
        if c.ty.align4() { size = (size + 3) & !3; }
        tp.cols.get_unchecked_mut(i).init(c.ty, size, c.name, c.notnull);
        size += c.ty.size();
      }
      size = (size + 3) & !3; // at last it should be aligned to keep the alignment of the next slot
      tp.init(id as u32, size.max(MIN_SLOT_SIZE as u16), c.cols.len() as u8);

      // handle table cons
      for cons in &c.cons {
        let (idx, _) = cols.get_full(cons.name).unchecked_unwrap();
        let ci = tp.cols.get_unchecked_mut(idx);
        match &cons.kind {
          TableConsKind::Primary => {
            ci.flags.set(ColFlags::PRIMARY, true);
            ci.flags.set(ColFlags::NOTNULL, true);
            if primary_cnt == 1 { ci.flags.set(ColFlags::UNIQUE, true); }
          }
          &TableConsKind::Foreign { table, col } => {
            let (f_ti_id, f_ti) = self.get_ti(table).unchecked_unwrap();
            let f_tp = self.get_page::<TablePage>(f_ti.meta as usize);
            let f_ci_id = f_tp.get_ci(col).unchecked_unwrap().0;
            ci.foreign_table = f_ti_id as u8;
            ci.foreign_col = f_ci_id as u8;
          }
          TableConsKind::Check(check) => {
            let (id, cp) = self.allocate_page::<CheckPage>();
            ci.check = id as u32;
            cp.len = check.len() as u32;
            let sz = ci.ty.size() as usize;
            let mut off = 0;
            for &c in check {
              debug_assert!(off + sz <= MAX_CHECK_BYTES);
              fill_ptr(cp.data.as_mut_ptr().add(off), ci.ty, c).unchecked_unwrap();
              off += sz;
            }
          }
        }
      }

      // update table info in meta page
      let ti = dp.tables.get_unchecked_mut(dp.table_num as usize);
      ti.meta = id as u32;
      ti.name_len = c.name.len() as u8;
      ti.name.as_mut_ptr().copy_from_nonoverlapping(c.name.as_ptr(), c.name.len());
      dp.table_num += 1;

      tp.cols_mut().iter_mut()
        .filter(|ci| ci.flags.contains(ColFlags::UNIQUE))
        .for_each(|ci| self.create_index_impl(ci));
      Ok(())
    }
  }

  pub fn drop_table(&mut self, name: &str) -> Result<()> {
    unsafe {
      let dp = self.get_page::<DbPage>(0);
      let id = self.get_ti(name)?.0;
      if self.has_foreign_link_to(id) { return Err(DropTableWithForeignLink(name.into())); }
      let meta = dp.tables.get_unchecked(id).meta;
      dp.tables.get_unchecked_mut(id).p().swap(dp.tables.get_unchecked_mut(dp.table_num as usize - 1));
      dp.table_num -= 1;
      let tp = self.get_page::<TablePage>(meta as usize);
      tp.cols_mut().iter_mut().filter(|ci| ci.index != !0).for_each(|ci| self.drop_index_impl(ci));
      let mut cur = tp.next;
      loop {
        // both TablePage and DataPage use [1] as next, [0] as prev
        let nxt = self.get_page::<(u32, u32)>(cur as usize).1;
        self.deallocate_page(cur as usize);
        cur = nxt;
        if cur == meta { break; }
      }
      Ok(())
    }
  }

  // `id` is index in DbPage::tables
  pub unsafe fn has_foreign_link_to(&mut self, id: usize) -> bool {
    let dp = self.get_page::<DbPage>(0);
    for ti in dp.tables() {
      let tp = self.get_page::<TablePage>(ti.meta as usize);
      for ci in tp.cols() {
        if ci.foreign_table == id as u8 { return true; }
      }
    }
    false
  }
}

impl Db {
  pub fn create_index(&mut self, table: &str, col: &str) -> Result<()> {
    unsafe {
      let (tp_id, tp) = self.get_tp(table)?;
      if self.record_iter((tp_id, tp)).any(|_| true) { return Err(CreateIndexOnNonEmpty(table.into())); }
      let ci = tp.get_ci(col)?.1;
      if ci.index != !0 { return Err(DupIndex(col.into())); }
      self.create_index_impl(ci);
      Ok(())
    }
  }

  unsafe fn create_index_impl(&mut self, ci: &mut ColInfo) {
    let (id, ip) = self.allocate_page::<IndexPage>();
    ci.index = id as u32;
    ip.init(true, ci.ty.size()); // it is the root, but also a leaf
  }

  pub fn drop_index(&mut self, table: &str, col: &str) -> Result<()> {
    unsafe {
      let ci = self.get_tp(table)?.1.get_ci(col)?.1;
      if ci.index == !0 { return Err(NoSuchIndex(col.into())); }
      if ci.flags.contains(ColFlags::UNIQUE) { return Err(DropIndexOnUnique(col.into())); }
      self.drop_index_impl(ci);
      Ok(())
    }
  }

  unsafe fn drop_index_impl(&mut self, ci: &mut ColInfo) {
    unsafe fn dfs(db: &mut Db, page: usize) {
      let ip = db.get_page::<IndexPage>(page);
      let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
      macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
      if !ip.leaf {
        for i in 0..ip.count as usize {
          dfs(db, at_ch!(i) as usize);
        }
      }
      db.deallocate_page(page);
    }
    dfs(self, ci.index as usize);
    ci.index = !0;
  }
}

impl Db {
  pub unsafe fn get_page<'a, P>(&mut self, page: usize) -> &'a mut P {
    debug_assert!(page < self.pages);
    (self.mmap.get_unchecked_mut(page * PAGE_SIZE).p() as *mut P).r()
  }

  // the return P is neither initialized nor zeroed, just keeping the original bytes
  // allocation may not always be successful(when 64G is used up), but in most cases this error is not recoverable, so let it crash
  pub unsafe fn allocate_page<'a, P>(&mut self) -> WithId<&'a mut P> {
    let dp = self.get_page::<DbPage>(0);
    let free = if dp.first_free != !0 {
      let free = dp.first_free as usize;
      dp.first_free = *self.get_page(free); // [0] stores next free(or none)
      free
    } else {
      self.file.set_len(((self.pages + 1) * PAGE_SIZE) as u64).unwrap_or_else(|e|
        panic!("Failed to allocate page because {}. The database may already be in an invalid state.", e));
      (self.pages, self.pages += 1).0
    };
    (free, self.get_page(free) as _)
  }

  pub unsafe fn deallocate_page(&mut self, page: usize) {
    let dp = self.get_page::<DbPage>(0);
    let first = self.get_page::<u32>(page);
    *first = dp.first_free;
    dp.first_free = page as u32;
  }

  // unsafe because return value's lifetime is arbitrary
  // return `id` is index in DbPage::tables
  pub unsafe fn get_ti<'a>(&mut self, table: &str) -> Result<WithId<&'a mut TableInfo>> {
    let dp = self.get_page::<DbPage>(0);
    match dp.pr().names().enumerate().find(|n| n.1 == table) {
      Some((idx, _)) => Ok((idx, dp.tables.get_unchecked_mut(idx))),
      None => Err(NoSuchTable(table.into())),
    }
  }

  // return `id` is page id
  pub unsafe fn get_tp<'a>(&mut self, table: &str) -> Result<WithId<&'a mut TablePage>> {
    let tp_id = self.get_ti(table)?.1.meta as usize;
    Ok((tp_id, self.get_page::<TablePage>(tp_id)))
  }

  pub unsafe fn allocate_data_slot(&mut self, tp_id: usize) -> Rid {
    let tp = self.get_page::<TablePage>(tp_id);
    if tp.first_free == !0 {
      let (id, dp) = self.allocate_page::<DataPage>();
      dp.init(tp.prev, tp_id as u32); // push back
      self.get_page::<(u32, u32)>(tp.prev as usize).1 = id as u32; // tp.prev.next, note that tp.prev may be a table/data page
      tp.prev = id as u32;
      tp.first_free = id as u32;
    }
    let free_page = tp.first_free;
    let dp = self.get_page::<DataPage>(free_page as usize);
    debug_assert!(dp.count < tp.cap);
    debug_assert!(tp.cap as usize <= MAX_SLOT_BS * 32);
    let slot = (0..tp.cap as usize).filter_map(|i| {
      let word = dp.used.get_unchecked_mut(i / 32);
      if (*word >> (i % 32)) == 0 { (*word |= 1 << (i % 32), Some(i)).1 } else { None }
    }).next().unchecked_unwrap() as u32;
    dp.count += 1;
    if dp.count == tp.cap { // full, move to next
      tp.first_free = dp.next_free;
    }
    Rid::new(free_page, slot)
  }

  pub unsafe fn deallocate_data_slot(&mut self, tp: &mut TablePage, rid: Rid) {
    let (page, slot) = (rid.page(), rid.slot());
    let dp = self.get_page::<DataPage>(page as usize);
    debug_assert_eq!((*dp.used.get_unchecked(slot as usize / 32) >> (slot % 32)) & 1, 1);
    *dp.used.get_unchecked_mut(slot as usize / 32) &= !(1 << (slot % 32));
    if dp.count == tp.cap { // not in free list, add it
      dp.next_free = tp.first_free;
      tp.first_free = page;
    }
    // it is never given back to db, for simplicity
    dp.count -= 1;
  }

  pub unsafe fn get_data_slot(&mut self, tp: &TablePage, rid: Rid) -> *mut u8 {
    let (page, slot) = (rid.page(), rid.slot());
    let off = (slot * tp.size as u32) as usize;
    self.get_page::<DataPage>(page as usize).data.as_mut_ptr().add(off)
  }
}