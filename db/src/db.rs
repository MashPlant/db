use std::{fs::{File, OpenOptions}, path::Path};
use memmap::{MmapOptions, MmapMut};

use physics::*;
use common::{*, Error::*, BareTy::*};
use syntax::ast::*;
use unchecked_unwrap::UncheckedUnwrap;
use crate::fill_ptr;

pub struct Db {
  pub(crate) mmap: MmapMut,
  pub(crate) pages: u32,
  pub(crate) file: File,
  pub(crate) path: String,
}

impl Db {
  pub fn create<'a>(path: impl AsRef<Path>) -> Result<'a, Db> {
    unsafe {
      let file = OpenOptions::new().read(true).write(true).create(true).append(true).open(path.as_ref())?;
      file.set_len(PAGE_SIZE as u64)?;
      // this is 64G, the maximum capacity of this db; mmap will not allocate memory unless accessed
      let mut mmap = MmapOptions::new().len(PAGE_SIZE * MAX_PAGE).map_mut(&file)?;
      (mmap.as_mut_ptr() as *mut DbPage).r().init();
      Ok(Db { mmap, pages: 1, file, path: path.as_ref().to_string_lossy().into_owned() })
    }
  }

  pub fn open<'a>(path: impl AsRef<Path>) -> Result<'a, Db> {
    unsafe {
      let file = OpenOptions::new().read(true).write(true).append(true).open(path.as_ref())?;
      let size = file.metadata()?.len() as usize;
      if size == 0 || size % PAGE_SIZE != 0 { return Err(InvalidSize(size)); }
      let mut mmap = MmapOptions::new().len(PAGE_SIZE * MAX_PAGE).map_mut(&file)?;
      let dp = (mmap.as_mut_ptr() as *mut DbPage).r();
      if &dp.magic != MAGIC { return Err(InvalidMagic(dp.magic)); }
      Ok(Db { mmap, pages: (size / PAGE_SIZE) as u32, file, path: path.as_ref().to_string_lossy().into_owned() })
    }
  }

  pub fn path(&self) -> &str { &self.path }
}

impl Db {
  pub unsafe fn dp<'a>(&mut self) -> &'a mut DbPage { self.get_page::<DbPage>(0) }

  pub fn create_table<'a>(&mut self, c: &CreateTable<'a>) -> Result<'a, ()> {
    unsafe {
      let dp = self.dp();

      // validate table and cols
      if dp.table_num == MAX_TABLE as u16 { return Err(TableExhausted); }
      if c.name.len() >= MAX_TABLE_NAME { return Err(TableNameTooLong(c.name)); }
      if self.get_tp(c.name).is_ok() { return Err(DupTable(c.name)); }
      if c.cols.len() >= MAX_COL { return Err(ColTooMany(c.cols.len())); }
      // its V will be used later to validate col cons, only allow one primary / foreign / check for one col
      let mut cols = IndexMap::default();
      for co in &c.cols {
        if cols.insert(co.name, (false, false, false, false)).is_some() { return Err(DupCol(co.name)); }
        if co.name.len() >= MAX_COL_NAME { return Err(ColNameTooLong(co.name)); }
      }

      // validate col cons
      let mut primary_cnt = 0;
      for cons in &c.cons {
        if let Some((idx, _, has_pfuc)) = cols.get_full_mut(cons.name) {
          match &cons.kind {
            TableConsKind::Primary => {
              if has_pfuc.0 { return Err(DupPrimary(cons.name)); }
              has_pfuc.0 = true;
              primary_cnt += 1;
            }
            &TableConsKind::Foreign { table, col } => {
              if has_pfuc.1 { return Err(DupForeign(cons.name)); }
              has_pfuc.1 = true;
              let ci = self.get_tp(table)?.1.get_ci(col)?;
              if !ci.flags.contains(ColFlags::UNIQUE) { return Err(ForeignKeyOnNonUnique(col)); }
              let (f_ty, ty) = (ci.ty, c.cols[idx].ty);
              // maybe too strict here, don't allow foreign link between two types or shorter string to longer string (for simplicity of future handling)
              match (f_ty.ty, ty.ty) {
                (VarChar, VarChar) if ty.size >= f_ty.size => {}
                (Bool, Bool) | (Int, Int) | (Float, Float) | (Date, Date) => {}
                _ => return Err(IncompatibleForeignTy { foreign: f_ty, own: ty }),
              }
            }
            TableConsKind::Unique => {
              if has_pfuc.2 { return Err(DupUnique(cons.name)); }
              has_pfuc.2 = true;
            }
            TableConsKind::Check(check) => {
              if has_pfuc.3 { return Err(DupCheck(cons.name)); }
              has_pfuc.3 = true;
              let ty = c.cols[idx].ty;
              let sz = ty.size() as usize;
              if check.len() * sz >= MAX_CHECK_BYTES { return Err(CheckTooLong(cons.name, check.len())); }
              let buf = Align4U8::new(ty.size() as usize); // dummy buffer
              for &c in check {
                if c.is_null() { return Err(CheckNull(cons.name)); } else { fill_ptr(buf.ptr, ty, c)?; }
              }
            }
          }
        } else { return Err(NoSuchCol(cons.name)); }
      }

      // validate size, the size is calculated in the same way as below
      let mut size = (c.cols.len() + 31) / 32 * 4; // null bitset
      for c in &c.cols {
        size += c.ty.size() as usize;
        if size > MAX_DATA_BYTE { return Err(ColSizeTooBig(size)); }
      }
      size = (size + 3) & !3; // it should be 4-aligned to keep the alignment of the next slot
      if size > MAX_DATA_BYTE { return Err(ColSizeTooBig(size)); }

      // now no error can occur, can write to db safely

      // handle each col def
      let (id, tp) = self.alloc_page::<TablePage>();
      let mut size = (c.cols.len() as u16 + 31) / 32 * 4; // null bitset
      for (i, c) in c.cols.iter().enumerate() {
        if c.ty.align4() { size = (size + 3) & !3; }
        tp.cols.get_unchecked_mut(i).init(c.ty, size, c.name, c.notnull);
        size += c.ty.size();
      }
      size = (size + 3) & !3; // at last it should be aligned to keep the alignment of the next slot
      tp.init(id, size.max(MIN_SLOT_SIZE as u16), c.cols.len() as u8, c.name);

      // handle table cons
      for cons in &c.cons {
        let (idx, _, _) = cols.get_full(cons.name).unchecked_unwrap();
        let ci = tp.cols.get_unchecked_mut(idx);
        match &cons.kind {
          TableConsKind::Primary => {
            ci.flags.set(ColFlags::PRIMARY, true);
            ci.flags.set(ColFlags::NOTNULL, true);
            if primary_cnt == 1 { ci.flags.set(ColFlags::UNIQUE, true); }
          }
          &TableConsKind::Foreign { table, col } => {
            let (f_tp_id, f_tp) = self.get_tp(table).unchecked_unwrap();
            let f_ci_id = f_tp.get_ci(col).unchecked_unwrap().idx(&f_tp.cols);
            ci.foreign_table = f_tp_id;
            ci.foreign_col = f_ci_id as u8;
          }
          TableConsKind::Unique => ci.flags.set(ColFlags::UNIQUE, true),
          TableConsKind::Check(check) => {
            let (id, cp) = self.alloc_page::<CheckPage>();
            ci.check = id;
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

      *dp.tables.get_unchecked_mut(dp.table_num as usize) = id;
      dp.table_num += 1;
      tp.cols().iter()
        .filter(|ci| ci.flags.contains(ColFlags::UNIQUE))
        .for_each(|ci| self.alloc_index(ci.pr()));
      Ok(())
    }
  }

  pub fn drop_table<'a>(&mut self, name: &'a str) -> Result<'a, ()> {
    unsafe {
      let dp = self.dp();
      for (idx, &tp_id) in dp.tables().iter().enumerate() {
        let tp = self.get_page::<TablePage>(tp_id);
        if tp.name() == name {
          if self.has_foreign_link_to(tp_id) { return Err(AlterTableWithForeignLink(name)); }
          let tables = dp.tables.as_mut_ptr();
          tables.add(idx).swap(tables.add(dp.table_num as usize - 1));
          dp.table_num -= 1;
          tp.cols().iter().filter(|ci| ci.index != !0).for_each(|ci| self.drop_index_impl(ci.pr()));
          let mut cur = tp.next;
          loop {
            // both TablePage and DataPage use [1] as next, [0] as prev
            let nxt = self.get_page::<(u32, u32)>(cur).1;
            self.dealloc_page(cur);
            cur = nxt;
            if cur == tp_id { break; }
          }
          return Ok(());
        }
      }
      Err(NoSuchTable(name))
    }
  }

  pub unsafe fn has_foreign_link_to(&mut self, tp_id: u32) -> bool {
    for &tp_id1 in self.dp().tables() {
      for ci in self.get_page::<TablePage>(tp_id1).cols() {
        if ci.foreign_table == tp_id { return true; }
      }
    }
    false
  }
}

impl Db {
  // this is pub for `index` crate's use
  pub unsafe fn alloc_index(&mut self, ci: &mut ColInfo) {
    let (id, ip) = self.alloc_page::<IndexPage>();
    ci.index = id;
    ip.init(true, ci.ty.size()); // it is the root, but also a leaf
  }

  pub fn drop_index<'a>(&mut self, table: &'a str, col: &'a str) -> Result<'a, ()> {
    unsafe {
      let ci = self.get_tp(table)?.1.get_ci(col)?;
      if ci.index == !0 { return Err(NoSuchIndex(col)); }
      if ci.flags.contains(ColFlags::UNIQUE) { return Err(DropIndexOnUnique(col)); }
      self.drop_index_impl(ci);
      Ok(())
    }
  }

  unsafe fn drop_index_impl(&mut self, ci: &mut ColInfo) {
    unsafe fn dfs(db: &mut Db, page: u32) {
      let ip = db.get_page::<IndexPage>(page);
      let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
      macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
      if !ip.leaf {
        for i in 0..ip.count as usize {
          dfs(db, at_ch!(i));
        }
      }
      db.dealloc_page(page);
    }
    dfs(self, ci.index);
    ci.index = !0;
  }
}

impl Db {
  pub unsafe fn get_page<'a, P>(&mut self, page: u32) -> &'a mut P {
    debug_assert!(page < self.pages);
    (self.mmap.get_unchecked_mut(page as usize * PAGE_SIZE).p() as *mut P).r()
  }

  // the return P is neither initialized nor zeroed, just keeping the original bytes
  // allocation may not always be successful(when 64G is used up), but in most cases this error is not recoverable, so let it crash
  pub unsafe fn alloc_page<'a, P>(&mut self) -> (u32, &'a mut P) {
    let dp = self.dp();
    let free = if dp.first_free != !0 {
      let free = dp.first_free;
      dp.first_free = *self.get_page(free); // [0] stores next free(or none)
      free
    } else {
      self.file.set_len(((self.pages as usize + 1) * PAGE_SIZE) as u64).unwrap_or_else(|e|
        panic!("Failed to allocate page because {}. The database may already be in an invalid state.", e));
      (self.pages, self.pages += 1).0
    };
    (free, self.get_page(free) as _)
  }

  pub unsafe fn dealloc_page(&mut self, page: u32) {
    let dp = self.dp();
    let first = self.get_page::<u32>(page);
    *first = dp.first_free;
    dp.first_free = page;
  }

  // for convenience, the index of TablePage is returned (because it cannot be obtained by `idx`)
  pub unsafe fn get_tp<'a, 'b>(&mut self, table: &'b str) -> Result<'b, (u32, &'a mut TablePage)> {
    for &tp_id in self.dp().tables() {
      let tp = self.get_page::<TablePage>(tp_id);
      if tp.name() == table { return Ok((tp_id, tp)); }
    }
    Err(NoSuchTable(table))
  }

  pub unsafe fn allocate_data_slot(&mut self, tp_id: u32) -> Rid {
    let tp = self.get_page::<TablePage>(tp_id);
    if tp.first_free == !0 {
      let (id, dp) = self.alloc_page::<DataPage>();
      dp.init(tp.prev, tp_id); // push back
      self.get_page::<(u32, u32)>(tp.prev).1 = id; // tp.prev.next, note that tp.prev may be a table/data page
      tp.prev = id;
      tp.first_free = id;
    }
    let free_page = tp.first_free;
    let dp = self.get_page::<DataPage>(free_page);
    debug_assert!(dp.count < tp.cap);
    debug_assert!(tp.cap as usize <= MAX_SLOT_BS * 32);
    let slot = (0..tp.cap as usize).filter_map(|i| {
      if bsget(dp.used.as_ptr(), i) { None } else { (bsset(dp.used.as_mut_ptr(), i), Some(i)).1 }
    }).next().unchecked_unwrap() as u32;
    dp.count += 1;
    if dp.count == tp.cap { // full, move to next
      tp.first_free = dp.next_free;
    }
    Rid::new(free_page, slot)
  }

  pub unsafe fn dealloc_data_slot(&mut self, tp: &mut TablePage, rid: Rid) {
    let (page, slot) = (rid.page(), rid.slot());
    let dp = self.get_page::<DataPage>(page);
    debug_assert!(bsget(dp.used.as_ptr(), slot as usize));
    bsdel(dp.used.as_mut_ptr(), slot as usize);
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
    self.get_page::<DataPage>(page).data.as_mut_ptr().add(off)
  }
}