use common::*;

#[repr(C)]
pub struct TableInfo {
  pub meta: u32,
  pub name_len: u8,
  pub name: [u8; MAX_TABLE_NAME as usize],
}

impl TableInfo {
  pub unsafe fn name<'a>(&self) -> &'a str {
    str_from_parts(self.name.as_ptr(), self.name_len as usize)
  }
}

#[repr(C)]
pub struct DbPage {
  pub magic: [u8; MAGIC_LEN],
  pub _rsv1: [u8; 2],
  // !0 for none
  pub first_free: u32,
  // using u8 here is not to save space(since there is still enough space in _rsv)
  // but to explicitly show that u8 is exactly enough
  pub table_num: u8,
  pub _rsv2: [u8; 39],
  // it is unpin, remove strategy = swap with last
  pub tables: [TableInfo; MAX_TABLE as usize],
}

pub const MAX_TABLE_NAME: u32 = 59;
pub const MAX_TABLE: u32 = 127;

impl DbPage {
  #[inline(always)]
  pub fn init(&mut self) {
    self.magic = *MAGIC;
    self.first_free = !0;
    self.table_num = 0;
  }

  pub unsafe fn names<'a>(&'a self) -> impl Iterator<Item=&'a str> + 'a {
    let table_num = self.table_num as usize;
    self.tables.iter().enumerate().filter_map(move |(i, ti)| { if i < table_num { Some(ti.name()) } else { None } })
  }

  #[inline(always)]
  pub unsafe fn tables<'a>(&self) -> &'a [TableInfo] {
    debug_assert!(self.table_num < MAX_TABLE as u8);
    std::slice::from_raw_parts(self.tables.as_ptr(), self.table_num as usize)
  }

  #[inline(always)]
  pub unsafe fn tables_mut<'a>(&mut self) -> &'a mut [TableInfo] {
    debug_assert!(self.table_num < MAX_TABLE as u8);
    std::slice::from_raw_parts_mut(self.tables.as_mut_ptr(), self.table_num as usize)
  }
}

#[cfg_attr(tarpaulin, skip)]
fn _ck() {
  const_assert_eq!(std::mem::size_of::<TableInfo>(), 64);
  const_assert_eq!(std::mem::size_of::<DbPage>(), common::PAGE_SIZE);
}