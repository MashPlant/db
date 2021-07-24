use std::{mem::size_of, slice};

use common::{*, Error::*};

bitflags::bitflags! {
  pub struct ColFlags: u8 {
    const PRIMARY = 0b1;
    const NOTNULL = 0b10;
    const UNIQUE = 0b100;
    const NOTNULL1 = Self::PRIMARY.bits | Self::NOTNULL.bits; // if any bits in NOTNULL1 exists, this slot can't be null
  }
}

#[repr(C)]
pub struct ColInfo {
  pub ty: ColTy,
  // index root page id, !0 for none
  pub index: u32,
  // `check >> 1` is check page id, `check == !0` for none
  // if `(check & 1) == 1`, the one-past-last item in check page is the default value
  pub check: u32,
  // index of TablePage, !0 for none
  pub f_table: u32,
  // index in TablePage::cols, if f_table == !0, f_col is meaningless
  pub f_col: u8,
  pub flags: ColFlags,
  // offset in a record; this is an important field, placing it so behind is to avoid the space of padding
  pub off: u16,
  // if `index == !0`, below 2 are meaningless
  // if `index != !0 && idx_name_len == 0`, it is an anonymous index (created by dbms, has no name)
  pub idx_name_len: u8,
  pub idx_name: [u8; MAX_IDX_NAME],
  pub name_len: u8,
  pub name: [u8; MAX_COL_NAME],
}

impl ColInfo {
  // `idx_name_len` and `idx_name` is not initialized here
  pub unsafe fn init(&mut self, ty: ColTy, off: u16, name: &str, notnull: bool) {
    self.ty = ty;
    self.off = off;
    self.index = !0;
    self.check = !0;
    self.name_len = name.len() as u8;
    self.name.as_mut_ptr().copy_from_nonoverlapping(name.as_ptr(), name.len());
    self.flags = if notnull { ColFlags::NOTNULL } else { ColFlags::empty() };
    self.f_table = !0;
  }

  pub unsafe fn name<'a>(&self) -> &'a str {
    str_from_parts(self.name.as_ptr(), self.name_len as usize)
  }

  pub unsafe fn idx_name<'a>(&self) -> Option<&'a str> {
    if self.index != !0 { Some(str_from_parts(self.idx_name.as_ptr(), self.idx_name_len as usize)) } else { None }
  }

  pub fn unique(&self, primary_cnt: usize) -> bool {
    self.flags.contains(ColFlags::UNIQUE) || (self.flags.contains(ColFlags::PRIMARY) && primary_cnt == 1)
  }
}

#[repr(C)]
pub struct TablePage {
  // !0 for none
  pub first: u32,
  // !0 for none
  pub first_free: u32,
  // there are at most (64G / 16) = 4G records, so u32 is enough
  pub count: u32,
  // the size of a single slot, including null-bitset and data
  pub size: u16,
  // always equal to MAX_DATA_BYTE / size, store it just to avoid division
  pub cap: u16,
  pub name_len: u8,
  pub name: [u8; MAX_TABLE_NAME],
  pub col_num: u8,
  pub cols: [ColInfo; MAX_COL],
}

pub const MAX_TABLE_NAME: usize = 46;
pub const MAX_COL_NAME: usize = 25;
pub const MAX_IDX_NAME: usize = 15;
pub const MAX_COL: usize = 127;

impl TablePage {
  pub unsafe fn init(&mut self, size: u16, col_num: u8, name: &str) {
    (self.first = !0, self.first_free = !0);
    self.count = 0;
    (self.size = size, self.cap = MAX_DATA_BYTE as u16 / size);
    self.name_len = name.len() as u8;
    self.name.as_mut_ptr().copy_from_nonoverlapping(name.as_ptr(), name.len());
    self.col_num = col_num;
  }

  pub unsafe fn name<'a>(&self) -> &'a str {
    str_from_parts(self.name.as_ptr(), self.name_len as usize)
  }

  pub unsafe fn cols<'a>(&self) -> &'a [ColInfo] {
    slice::from_raw_parts(self.cols.as_ptr(), self.col_num as usize)
  }

  pub unsafe fn primary_cols<'a>(&self) -> impl Iterator<Item=&'a ColInfo> {
    self.cols().iter().filter(|ci| ci.flags.contains(ColFlags::PRIMARY))
  }

  pub unsafe fn get_ci<'a, 'b>(&mut self, col: &'b str) -> Result<'b, &'a mut ColInfo> {
    match self.pr().cols().iter().map(|c| c.name()).enumerate().find(|n| n.1 == col) {
      Some((idx, _)) => Ok(self.pr().cols.get_unchecked_mut(idx)),
      None => Err(NoSuchCol(col)),
    }
  }
}

#[cfg_attr(tarpaulin, ignore)]
fn _ck() {
  const_assert_eq!(size_of::<ColInfo>(), 64);
  const_assert_eq!(size_of::<TablePage>(), common::PAGE_SIZE);
}