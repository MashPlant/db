use std::{fs::{File, OpenOptions}, path::Path};
use memmap::{MmapOptions, MmapMut};
use unchecked_unwrap::UncheckedUnwrap;

use physics::*;
use common::{*, Error::*};
use syntax::ast::*;
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
      if c.table.len() > MAX_TABLE_NAME { return Err(TableNameTooLong(c.table)); }
      if self.get_tp(c.table).is_ok() { return Err(DupTable(c.table)); }
      if c.cols.len() > MAX_COL { return Err(ColTooMany(c.cols.len())); }
      // its V will be used later to validate col cons, only allow one primary / foreign / check for one col
      let mut cols = IndexMap::default();
      for cd in &c.cols {
        if cols.insert(cd.col, (false, false, false, false)).is_some() { return Err(DupCol(cd.col)); }
        if cd.col.len() > MAX_COL_NAME { return Err(ColNameTooLong(cd.col)); }
      }

      // validate col cons
      let mut primary_cnt = 0;
      for cons in &c.cons {
        match cons {
          ColCons::Primary(cols1) => for col in cols1 {
            let (_, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.0, has_pfuc.0 = true).0 { return Err(DupConstraint(col)); }
            primary_cnt += 1;
          }
          ColCons::Foreign { col, f_table, f_col } => {
            let (idx, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.1, has_pfuc.1 = true).0 { return Err(DupConstraint(col)); }
            let f_ci = self.get_tp(f_table)?.1.get_ci(f_col)?;
            if !f_ci.flags.contains(ColFlags::UNIQUE) { return Err(ForeignOnNotUnique(f_col)); }
            let (foreign, own) = (f_ci.ty, c.cols.get_unchecked(idx).ty);
            if foreign != own { return Err(IncompatibleForeignTy { foreign, own }); };
          }
          ColCons::Unique(col) => {
            let (_, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.2, has_pfuc.2 = true).0 { return Err(DupConstraint(col)); }
          }
          ColCons::Check(col, check) => {
            let (idx, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.3, has_pfuc.3 = true).0 { return Err(DupConstraint(col)); }
            let cd = c.cols.get_unchecked(idx);
            let sz = cd.ty.size() as usize;
            // default value will use one slot in check page
            if sz * (check.len() + (cd.dft.is_some() as usize)) > MAX_CHECK_BYTES { return Err(CheckTooLong(col)); }
            let buf = Align4U8::new(sz); // dummy buffer, only for typeck
            for &c in check {
              if c.is_null() { return Err(CheckNull(col)); } else { fill_ptr(buf.ptr, cd.ty, c)?; }
            }
          }
        }
      }
      for col in &c.cols {
        if let Some(dft) = col.dft {
          if !dft.is_null() {
            fill_ptr(Align4U8::new(col.ty.size() as usize).ptr, col.ty, dft)?;
          }
        }
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
        tp.cols.get_unchecked_mut(i).init(c.ty, size, c.col, c.notnull);
        size += c.ty.size();
      }
      size = (size + 3) & !3; // at last it should be aligned to keep the alignment of the next slot
      tp.init(size.max(MIN_SLOT_SIZE as u16), c.cols.len() as u8, c.table);

      // handle table cons
      for cons in &c.cons {
        match cons {
          ColCons::Primary(pks) => for col in pks {
            let ci = tp.cols.get_unchecked_mut(cols.get_full(col).unchecked_unwrap().0);
            ci.flags.set(ColFlags::PRIMARY, true);
            ci.flags.set(ColFlags::NOTNULL, true);
            if primary_cnt == 1 { ci.flags.set(ColFlags::UNIQUE, true); }
          }
          ColCons::Foreign { col, f_table, f_col } => {
            let ci = tp.cols.get_unchecked_mut(cols.get_full(col).unchecked_unwrap().0);
            let (f_tp_id, f_tp) = self.get_tp(f_table).unchecked_unwrap();
            let f_ci_id = f_tp.get_ci(f_col).unchecked_unwrap().idx(&f_tp.cols);
            (ci.f_table = f_tp_id, ci.f_col = f_ci_id as u8);
          }
          ColCons::Unique(col) => {
            let ci = tp.cols.get_unchecked_mut(cols.get_full(col).unchecked_unwrap().0);
            ci.flags.set(ColFlags::UNIQUE, true);
          }
          ColCons::Check(col, check) => {
            let ci = tp.cols.get_unchecked_mut(cols.get_full(col).unchecked_unwrap().0);
            let (id, cp) = self.alloc_page::<CheckPage>();
            ci.check = id << 1;
            cp.count = check.len() as u16;
            let sz = ci.ty.size() as usize;
            for (idx, &c) in check.iter().enumerate() {
              fill_ptr(cp.data.as_mut_ptr().add(idx * sz), ci.ty, c).unchecked_unwrap();
            }
          }
        }
      }
      for (idx, col) in c.cols.iter().enumerate() {
        if let Some(dft) = col.dft {
          if !dft.is_null() {
            let ci = tp.cols.get_unchecked_mut(idx);
            let cp = if ci.check == !0 {
              let (id, cp) = self.alloc_page::<CheckPage>();
              ci.check = id << 1;
              (cp.count = 0, cp).1
            } else { self.get_page::<CheckPage>(ci.check >> 1) };
            ci.check |= 1;
            fill_ptr(cp.data.as_mut_ptr().add(cp.count as usize * ci.ty.size() as usize), ci.ty, dft).unchecked_unwrap();
          }
        }
      }

      *dp.tables.get_unchecked_mut(dp.table_num as usize) = id;
      dp.table_num += 1;
      tp.cols().iter()
        .filter(|ci| ci.flags.contains(ColFlags::UNIQUE) || ci.f_table != !0)
        .for_each(|ci| self.alloc_index(ci.pr(), "").unchecked_unwrap());
      Ok(())
    }
  }

  // return all the (tp_id1, ci_id1, ci_id), where tp_id1.ci_id1 has foreign link to tp_id.ci_id
  pub unsafe fn foreign_links_to(&mut self, tp_id: u32) -> Vec<(u32, u8, u8)> {
    self.dp().tables().iter().flat_map(|&tp_id1|
      self.get_page::<TablePage>(tp_id1).cols().iter().enumerate().filter_map(move |(ci_id1, ci1)|
        if ci1.f_table == tp_id { Some((tp_id1, ci_id1 as u8, ci1.f_col)) } else { None })).collect()
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
      self.file.set_len((self.pages as u64 + 1) * PAGE_SIZE as u64).unwrap_or_else(|e| panic!("Failed to allocate page because {}. The database may already be in an invalid state.", e));
      (self.pages, self.pages += 1).0
    };
    (free, self.get_page(free))
  }

  // add `page` to the head of free list
  pub unsafe fn dealloc_page(&mut self, page: u32) {
    let dp = self.dp();
    *self.get_page::<u32>(page) = dp.first_free;
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
      (dp.init(tp.first), tp.first = id); // push front, so insert order may not be kept
      tp.first_free = id;
    }
    let free = tp.first_free;
    let dp = self.get_page::<DataPage>(free);
    debug_assert!(dp.count < tp.cap);
    let slot = (0..tp.cap as usize).filter_map(|i| {
      if bsget(dp.used.as_ptr(), i) { None } else { (bsset(dp.used.as_mut_ptr(), i), Some(i)).1 }
    }).next().unchecked_unwrap() as u32;
    dp.count += 1;
    if dp.count == tp.cap { tp.first_free = dp.next_free; }
    Rid::new(free, slot)
  }

  pub unsafe fn dealloc_data_slot(&mut self, tp: &mut TablePage, rid: Rid) {
    let (page, slot) = (rid.page(), rid.slot());
    let dp = self.get_page::<DataPage>(page);
    debug_assert!(bsget(dp.used.as_ptr(), slot as usize));
    bsdel(dp.used.as_mut_ptr(), slot as usize);
    if dp.count == tp.cap { // not in free list, add it
      (dp.next_free = tp.first_free, tp.first_free = page);
    }
    // it is never given back to db, for simplicity (this enables calling `dealloc_data_slot` during iteration)
    dp.count -= 1;
  }

  pub unsafe fn get_data_slot(&mut self, tp: &TablePage, rid: Rid) -> *mut u8 {
    self.get_page::<DataPage>(rid.page()).data.as_mut_ptr().add((rid.slot() * tp.size as u32) as usize)
  }
}