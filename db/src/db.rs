use std::{fs::{File, OpenOptions}, path::Path};
use memmap::{MmapOptions, MmapMut};
use unchecked_unwrap::UncheckedUnwrap;
use chrono::NaiveDate;

use physics::*;
use common::{*, Error::*, BareTy::*};
use syntax::ast::*;

pub struct Db {
  pub(crate) mmap: MmapMut,
  pub(crate) file: File,
  pub(crate) lob_mmap: MmapMut,
  pub(crate) lob_file: File,
  pub(crate) pages: u32,
  pub(crate) lob_slots: u32,
}

impl Db {
  pub fn create<'a>(path: impl AsRef<Path>) -> Result<'a, Db> {
    unsafe {
      let opt = OpenOptions::new().read(true).write(true).create(true).append(true).clone();
      let file = opt.open(path.as_ref())?;
      file.set_len(PAGE_SIZE as u64)?;
      // this is 64G, the maximum capacity of this db; mmap will not allocate memory unless accessed
      let mut mmap = MmapOptions::new().len(PAGE_SIZE * MAX_PAGE).map_mut(&file)?;
      (mmap.as_mut_ptr() as *mut DbPage).r().init();
      let lob_file = opt.open(path.as_ref().with_extension(LOB_SUFFIX))?;
      lob_file.set_len(LOB_SLOT_SIZE as u64)?;
      // lob file can use all the 32 bits addr space, each addr for 32 bytes, in all 128G
      let mut lob_mmap = MmapOptions::new().len(!0u32 as usize * LOB_SLOT_SIZE).map_mut(&lob_file)?;
      (lob_mmap.as_mut_ptr() as *mut FreeLobSlot).r().init_nil();
      Ok(Db { mmap, file, lob_mmap, lob_file, pages: 1, lob_slots: 1 })
    }
  }

  pub fn open<'a>(path: impl AsRef<Path>) -> Result<'a, Db> {
    unsafe {
      let opt = OpenOptions::new().read(true).write(true).append(true).clone();
      let file = opt.open(path.as_ref())?;
      let size = file.metadata()?.len() as usize;
      if size == 0 || size % PAGE_SIZE != 0 { return Err(InvalidSize { size, expect_multiply_of: PAGE_SIZE }); }
      let mmap = MmapOptions::new().len(PAGE_SIZE * MAX_PAGE).map_mut(&file)?;
      let dp = &*(mmap.as_ptr() as *const DbPage);
      if &dp.magic != MAGIC { return Err(InvalidMagic(dp.magic)); }
      let lob_file = opt.open(path.as_ref().with_extension(LOB_SUFFIX))?;
      let lob_size = lob_file.metadata()?.len() as usize;
      if lob_size == 0 || lob_size % LOB_SLOT_SIZE != 0 { return Err(InvalidSize { size: lob_size, expect_multiply_of: LOB_SLOT_SIZE }); }
      let lob_mmap = MmapOptions::new().len(!0u32 as usize * LOB_SLOT_SIZE).map_mut(&lob_file)?;
      Ok(Db { mmap, file, lob_file, lob_mmap, pages: (size / PAGE_SIZE) as u32, lob_slots: (lob_size / LOB_SLOT_SIZE) as u32 })
    }
  }
}

impl Db {
  // like `lit2ptr`, but only do type check
  pub fn lit2ptr_ck(ty: FixTy, val: CLit) -> Result<()> {
    match (ty.ty, val.lit()) {
      (Bool, Lit::Bool(_)) => Ok(()),
      (Int, Lit::Number(_)) => Ok(()),
      (Float, Lit::Number(_)) => Ok(()),
      (Date, Lit::Str(v)) => (crate::date(v)?, Ok(())).1,
      (Date, Lit::Date(_)) => Ok(()),
      (Char, Lit::Str(v)) if v.len() <= ty.size as usize => Ok(()),
      _ => Err(ColLitMismatch { ty: ColTy::FixTy(ty), val }),
    }
  }

  // ignore non-varchar case
  pub fn varchar_ck(ty: ColTy, val: CLit) -> Result<()> {
    match (ty, val.lit()) {
      (varchar!(size), Lit::Str(v)) if v.len() <= size as usize => Ok(()),
      (varchar!(), _) => Err(ColLitMismatch { ty, val }),
      _ => Ok(())
    }
  }

  // `ptr` points to the location in this record where `val` should locate, not the start address of data slot
  // if `val` is null, it is always regarded as illegal
  // Varchar case is not handled here, use `lit2varchar` to write varchar to ptr
  pub unsafe fn lit2ptr<'a>(&mut self, ptr: *mut u8, ty: FixTy, val: CLit<'a>) -> Result<'a, ()> {
    Ok(match (ty.ty, val.lit()) {
      (Bool, Lit::Bool(v)) => *(ptr as *mut bool) = v,
      (Int, Lit::Number(v)) => *(ptr as *mut i32) = v as i32,
      (Float, Lit::Number(v)) => *(ptr as *mut f32) = v as f32,
      (Date, Lit::Str(v)) => *(ptr as *mut NaiveDate) = crate::date(v)?,
      (Date, Lit::Date(v)) => *(ptr as *mut NaiveDate) = v, // it is not likely to enter this case, because parser cannot produce Date
      (Char, Lit::Str(v)) if v.len() <= ty.size as usize => {
        *ptr = v.len() as u8;
        ptr.add(1).copy_from_nonoverlapping(v.as_ptr(), v.len());
      }
      _ => return Err(ColLitMismatch { ty: ColTy::FixTy(ty), val })
    })
  }

  // if `ptr`'s content doesn't have initial value (e.g.: insert), set initialized = false, otherwise set initialized = true; this helps handling varchar
  pub unsafe fn lit2varchar(&mut self, ptr: *mut u8, s: &str, initialized: bool) {
    let write_varchar = |db: &mut Db| {
      let (lob_id, cap, ptr1) = db.alloc_lob(s.len() as u32);
      ptr1.copy_from_nonoverlapping(s.as_ptr(), s.len());
      *(ptr as *mut VarcharSlot) = VarcharSlot { lob_id, len: s.len() as u16, cap: cap as u16 };
    };
    if initialized {
      let old = (ptr as *mut VarcharSlot).r();
      if s.len() <= old.cap as usize {
        old.len = s.len() as u16;
        self.get_lob(old.lob_id).copy_from_nonoverlapping(s.as_ptr(), s.len());
      } else {
        self.dealloc_lob(old.lob_id, old.cap as u32);
        write_varchar(self);
      }
    } else { write_varchar(self); }
  }

  // input the whole data slot, result may be null
  pub unsafe fn data2lit<'a>(&self, data: *const u8, ci_id: u32, ci: &ColInfo) -> CLit<'a> {
    if bsget(data as *const u32, ci_id as usize) { return CLit::new(Lit::Null); };
    self.ptr2lit(data.add(ci.off as usize), ci.ty)
  }

  // input the data ptr, result is never null
  pub unsafe fn ptr2lit<'a>(&self, ptr: *const u8, ty: ColTy) -> CLit<'a> {
    CLit::new(match ty {
      bool!() => Lit::Bool(*(ptr as *const bool)),
      int!() => Lit::Number(*(ptr as *const i32) as f64),
      float!() => Lit::Number(*(ptr as *const f32) as f64),
      date!() => Lit::Date(*(ptr as *const NaiveDate)),
      char!() => Lit::Str(str_from_db(ptr)),
      varchar!() => Lit::Str(self.varchar(ptr)),
    })
  }

  pub unsafe fn varchar<'a>(&self, ptr: *const u8) -> &'a str {
    let v = (ptr as *const VarcharSlot).r();
    str_from_parts(self.pr().get_lob(v.lob_id), v.len as usize)
  }

  pub unsafe fn free_varchar(&mut self, ptr: *const u8) {
    let v = (ptr as *const VarcharSlot).r();
    self.dealloc_lob(v.lob_id, v.cap as u32);
  }
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
      if c.cols.is_empty() { return Err(ColTooFew); }
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
            let (idx, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.0, has_pfuc.0 = true).0 { return Err(DupConstraint(col)); }
            if c.cols.get_unchecked(idx).ty.is_varchar() { return Err(UnsupportedVarcharOp(col)); }
            primary_cnt += 1;
          }
          ColCons::Foreign { col, f_table, f_col } => {
            let (idx, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.1, has_pfuc.1 = true).0 { return Err(DupConstraint(col)); }
            let cd = c.cols.get_unchecked(idx);
            let f_tp = self.get_tp(f_table)?.1;
            let f_ci = f_tp.get_ci(f_col)?;
            if !f_ci.unique(f_tp.primary_cols().count()) { return Err(ForeignOnNotUnique(f_col)); }
            debug_assert!(!f_ci.ty.is_varchar());
            if f_ci.ty != cd.ty { return Err(IncompatibleForeignTy { foreign: f_ci.ty, own: cd.ty }); }
          }
          ColCons::Unique(col) => {
            let (idx, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.2, has_pfuc.2 = true).0 { return Err(DupConstraint(col)); }
            if c.cols.get_unchecked(idx).ty.is_varchar() { return Err(UnsupportedVarcharOp(col)); }
          }
          ColCons::Check(col, check) => {
            let (idx, _, has_pfuc) = if let Some(x) = cols.get_full_mut(col) { x } else { return Err(NoSuchCol(col)); };
            if (has_pfuc.3, has_pfuc.3 = true).0 { return Err(DupConstraint(col)); }
            let cd = c.cols.get_unchecked(idx);
            if cd.ty.is_varchar() { return Err(UnsupportedVarcharOp(col)); }
            let sz = cd.ty.size() as usize;
            // default value will use one slot in check page
            if sz * (check.len() + (cd.dft.is_some() as usize)) > MAX_CHECK_BYTES { return Err(CheckTooLong(col)); }
            for &c in check {
              if c.is_null() { return Err(CheckNull(col)); } else { Db::lit2ptr_ck(cd.ty.fix_ty(), c)?; }
            }
          }
        }
      }
      for cd in &c.cols {
        if let Some(dft) = cd.dft {
          if cd.ty.is_varchar() { return Err(UnsupportedVarcharOp(cd.col)); }
          // you can set default = null to a notnull col, such insertion will be rejected though
          if !dft.is_null() { Db::lit2ptr_ck(cd.ty.fix_ty(), dft)?; }
        }
      }

      // validate size, the size is calculated in the same way as below
      let mut size = (c.cols.len() + 31) / 32 * 4; // null bitset
      for c in &c.cols { size += c.ty.size() as usize; }
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
      size = (size + 3) & !3;
      tp.init(size.max(MIN_SLOT_SIZE as u16), c.cols.len() as u8, c.table);

      // handle table cons
      for cons in &c.cons {
        match cons {
          ColCons::Primary(pks) => for col in pks {
            let ci = tp.cols.get_unchecked_mut(cols.get_full(col).unchecked_unwrap().0);
            ci.flags.set(ColFlags::PRIMARY, true);
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
              self.lit2ptr(cp.data.as_mut_ptr().add(idx * sz), ci.ty.fix_ty(), c).unchecked_unwrap();
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
            self.lit2ptr(cp.data.as_mut_ptr().add(cp.count as usize * ci.ty.size() as usize), ci.ty.fix_ty(), dft).unchecked_unwrap();
          }
        }
      }

      *dp.tables.get_unchecked_mut(dp.table_num as usize) = id;
      dp.table_num += 1;
      tp.cols().iter().filter(|ci| ci.unique(primary_cnt) || ci.f_table != !0).for_each(|ci| self.alloc_index(ci.pr(), "").unchecked_unwrap());
      Ok(())
    }
  }

  // return all the (tp_id1, ci_id1, ci_id), where tp_id1.ci_id1 has foreign link to tp_id.ci_id
  pub unsafe fn foreign_links_to<'a>(&'a mut self, tp_id: u32) -> impl Iterator<Item=(u32, u8, u8)> + 'a {
    self.dp().tables().iter().flat_map(move |&tp_id1|
      self.get_page::<TablePage>(tp_id1).cols().iter().enumerate().filter_map(move |(ci_id1, ci1)|
        if ci1.f_table == tp_id { Some((tp_id1, ci_id1 as u8, ci1.f_col)) } else { None }))
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
      self.file.set_len((self.pages as u64 + 1) * PAGE_SIZE as u64).expect("Failed to allocate page. The database may already be in an invalid state.");
      (self.pages, self.pages += 1).0
    };
    (free, self.get_page(free))
  }

  // add `page` to the head of free list
  pub unsafe fn dealloc_page(&mut self, page: u32) {
    debug_assert!(page < self.pages);
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

  pub unsafe fn alloc_data_slot(&mut self, tp_id: u32) -> Rid {
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