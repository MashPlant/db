use std::{fs::{File, OpenOptions}, path::Path, mem};
use memmap::{MmapOptions, MmapMut};

use physics::*;
use common::{*, Error::*};
use syntax::ast::*;

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

  #[inline(always)]
  pub fn path(&self) -> &str { &self.path }
}

impl Db {
  pub fn create_table(&mut self, c: &CreateTable) -> Result<()> {
    unsafe {
      let dp = self.get_page::<DbPage>(0);
      self.validate_table(c, &*dp)?;
      let (id, tp) = self.allocate_page::<TablePage>();
      let mut size = (c.cols.len() as u16 + 31) / 32 * 4; // null bitset
      for (i, c) in c.cols.iter().enumerate() {
        if c.ty.align4() { size = (size + 3) & !3; }
        let ci = tp.cols.get_unchecked_mut(i);
        ci.ty = c.ty;
        ci.off = size;
        ci.index = !0;
        ci.name_len = c.name.len() as u8;
        ci.name.as_mut_ptr().copy_from_nonoverlapping(c.name.as_ptr(), c.name.len());
        ci.flags.set(ColFlags::NOTNULL, c.notnull); // todo: primary also implies notnull
        ci.foreign_table = !0;
        // todo: many fields needing to fill, including index
        size += c.ty.size();
        if size as usize > MAX_DATA_BYTE { return Err(ColSizeTooBig(size as usize)); }
      }
      size = (size + 3) & !3; // at last it should be aligned to keep the alignment of the next slot
      if size as usize > MAX_DATA_BYTE { return Err(ColSizeTooBig(size as usize)); }
      tp.init(id as u32, size.max(MIN_SLOT_SIZE as u16), c.cols.len() as u8);
      let ti = dp.tables.get_unchecked_mut(dp.table_num as usize);
      ti.meta = id as u32;
      ti.name_len = c.name.len() as u8;
      ti.name.as_mut_ptr().copy_from_nonoverlapping(c.name.as_ptr(), c.name.len());
      dp.table_num += 1;
      Ok(())
    }
  }

  unsafe fn validate_table(&mut self, c: &CreateTable, dp: &DbPage) -> Result<()> {
    if dp.table_num == MAX_TABLE as u8 { return Err(TableExhausted); }
    if c.name.len() as u32 >= MAX_TABLE_NAME { return Err(TableNameTooLong(c.name.into())); }
    if dp.names().find(|&name| name == c.name).is_some() { return Err(DupTable(c.name.into())); }
    if c.cols.len() >= MAX_COL as usize { return Err(ColTooMany(c.cols.len())); }
    let mut cols = HashSet::new();
    for co in &c.cols {
      if cols.contains(co.name) { return Err(DupCol(co.name.into())); }
      cols.insert(co.name);
      if co.name.len() as u32 >= MAX_COL_NAME { return Err(ColNameTooLong(co.name.into())); }
    }
    Ok(())
  }

  pub fn drop_table(&mut self, name: &str) -> Result<()> {
    unsafe {
      let dp = self.get_page::<DbPage>(0);
      let idx = self.get_ti(name)?.p().offset_from(dp.tables.as_ptr()) as usize;
      let meta = dp.tables.get_unchecked(idx).meta;
      dp.tables.get_unchecked_mut(idx).p().swap(dp.tables.get_unchecked_mut(dp.table_num as usize - 1));
      dp.table_num -= 1;
      let tp = self.get_page::<TablePage>(meta as usize);
      let mut cur = tp.next;
      loop {
        // both TablePage and DataPage use [1] as next, [0] as prev
        let nxt = self.get_page::<(u32, u32)>(cur as usize).1;
        self.deallocate_page(cur as usize);
        cur = nxt;
        if cur == meta { break; }
      }
      // todo: drop index
      Ok(())
    }
  }
}

impl Db {
  pub fn create_index(&mut self, table: &str, col: &str) -> Result<()> {
    unsafe {
      let tp = self.get_ti(table)?.meta as usize;
      let tp = self.get_page::<TablePage>(tp) ;
      if self.record_iter(tp).count() != 0 { return Err(CreateIndexOnNonEmpty(table.into())); }
      let ci = tp.get_ci(col)?;
      if ci.index != !0 { return Err(DupIndex(col.into())); }
      let (id, ip) = self.allocate_page::<IndexPage>();
      ci.index = id as u32;
      ip.init(true, ci.ty.size()); // it is the root, but also a leaf
      Ok(())
    }
  }

  pub fn drop_index(&mut self, table: &str, col: &str) -> Result<()> {
    unsafe {
      let ci = self.get_ci(table, col)?;
      if ci.index == !0 { return Err(NoSuchIndex(col.into())); }
      // todo free all pages used by index
      Ok(())
    }
  }
}

impl Db {
  #[inline(always)]
  pub unsafe fn get_page<'a, P>(&mut self, page: usize) -> &'a mut P {
    debug_assert!(page < self.pages);
    (self.mmap.get_unchecked_mut(page * PAGE_SIZE).p() as *mut P).r()
  }

  // the return P is neither initialized nor zeroed, just keeping the original bytes
  // allocation may not always be successful(when 64G is used up), but in most cases this error is not recoverable, so let it crash
  #[inline(always)]
  pub unsafe fn allocate_page<'a, P>(&mut self) -> (usize, &'a mut P) {
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

  #[inline(always)]
  pub unsafe fn deallocate_page(&mut self, page: usize) {
    let dp = self.get_page::<DbPage>(0);
    let first = self.get_page::<u32>(page);
    *first = dp.first_free;
    dp.first_free = page as u32;
  }

  // unsafe because return value's lifetime is arbitrary
  #[inline(always)]
  pub unsafe fn get_ti<'a>(&mut self, table: &str) -> Result<&'a mut TableInfo> {
    let dp = self.get_page::<DbPage>(0);
    match dp.pr().names().enumerate().find(|n| n.1 == table) {
      Some((idx, _)) => Ok(dp.tables.get_unchecked_mut(idx)),
      None => Err(NoSuchTable(table.into())),
    }
  }

  #[inline(always)]
  pub unsafe fn id_of(&self, tp: &TablePage) -> usize {
    (tp as *const TablePage).offset_from(self.mmap.as_ptr() as *const TablePage) as usize
  }

  #[inline(always)]
  pub unsafe fn get_ci<'a>(&mut self, table: &str, col: &str) -> Result<&'a mut ColInfo> {
    let meta = self.get_ti(table)?.meta as usize;
    self.get_page::<TablePage>(meta).get_ci(col)
  }

  pub unsafe fn allocate_data_slot(&mut self, tp: &mut TablePage) -> Rid {
    let table_page = self.id_of(tp) as u32;
    if tp.first_free == !0 {
      let (id, dp) = self.allocate_page::<DataPage>();
      dp.init(tp.prev, table_page); // push back
      tp.first_free = id as u32;
    }
    let free_page = tp.first_free;
    let dp = self.get_page::<DataPage>(free_page as usize);
    debug_assert!(dp.count < tp.cap);
    debug_assert!(((tp.cap + 31) / 32) as usize <= MAX_SLOT_BS);
    let mut slot = mem::MaybeUninit::<u32>::uninit();
    'out: for i in 0..((tp.cap + 31) / 32) as usize {
      let x = dp.used.get_unchecked_mut(i);
      if *x != !0 {
        for b in 0..32 {
          if ((*x >> b) & 1) == 0 {
            *x |= 1 << b;
            slot.as_mut_ptr().write(i as u32 * 32 + b);
            break 'out;
          }
        }
        debug_unreachable!();
      }
    }
    dp.count += 1;
    if dp.count == tp.cap { // full, move to next
      tp.first_free = dp.next_free;
    }
    Rid::new(free_page, slot.assume_init())
  }

  pub unsafe fn deallocate_data_slot(&mut self, tp: &mut TablePage, rid: Rid) {
    let (page, slot) = (rid.page(), rid.slot());
    let dp = self.get_page::<DataPage>(page as usize);
    debug_assert_eq!((*dp.used.get_unchecked(slot as usize / 32) >> (slot % 32)) & 1, 1);
    *dp.used.get_unchecked_mut(slot as usize / 32) &= !(1 << (slot % 32));
    if dp.count == tp.cap { // not in free list, add it
      dp.next_free = tp.first_free;
      tp.first_free = page;
    } else if dp.count == 1 {
      // todo: free it, give back to db
    }
    dp.count -= 1;
  }

  #[inline(always)]
  pub unsafe fn get_data_slot(&mut self, tp: &TablePage, rid: Rid) -> *mut u8 {
    let (page, slot) = (rid.page(), rid.slot());
    let off = (slot * tp.size as u32) as usize;
    self.get_page::<DataPage>(page as usize).data.as_mut_ptr().add(off)
  }
}