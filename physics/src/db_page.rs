use common::*;

#[repr(C)]
pub struct DbPage {
  pub magic: [u8; MAGIC_LEN],
  pub _rsv1: [u8; 2],
  // !0 for none
  pub first_free: u32,
  // using u16 here is not to save space (since there is still enough space in _rsv)
  // but to explicitly show that u16 is enough
  pub table_num: u16,
  pub _rsv2: [u8; 2],
  pub tables: [u32; MAX_TABLE],
}

pub const MAX_TABLE: usize = 2041;

impl DbPage {
  pub fn init(&mut self) {
    self.magic = *MAGIC;
    self.first_free = !0;
    self.table_num = 0;
  }

  pub unsafe fn tables<'a>(&self) -> &'a [u32] {
    debug_assert!(self.table_num < MAX_TABLE as u16);
    std::slice::from_raw_parts(self.tables.as_ptr(), self.table_num as usize)
  }
}

#[cfg_attr(tarpaulin, skip)]
fn _ck() { const_assert_eq!(std::mem::size_of::<DbPage>(), common::PAGE_SIZE); }