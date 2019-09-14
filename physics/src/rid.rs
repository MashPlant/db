use std::fmt;
use common::{MAX_PAGE, MAX_SLOT, LOG_MAX_SLOT};

// (32 - LOG_MAX_SLOT) bits for page, LOG_MAX_SLOT bits for slot
// enough to locate both a record in DataPage and a col_info in TablePage
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(C)]
pub struct Rid(u32);

impl Rid {
  #[inline(always)]
  pub fn new(page: u32, slot: u32) -> Rid {
    debug_assert!(page < (MAX_PAGE as u32));
    debug_assert!(slot < (MAX_SLOT as u32));
    Rid((page << (LOG_MAX_SLOT as u32)) | slot)
  }

  #[inline(always)]
  pub fn page(self) -> u32 { self.0 >> (LOG_MAX_SLOT as u32) }
  #[inline(always)]
  pub fn slot(self) -> u32 { self.0 & ((MAX_SLOT as u32) - 1) }
  #[inline(always)]
  pub fn set_page(&mut self, page: u32) -> &mut Self {
    debug_assert!(page < (MAX_PAGE as u32));
    self.0 &= (MAX_SLOT as u32) - 1;
    self.0 |= page << (LOG_MAX_SLOT as u32);
    self
  }
  #[inline(always)]
  pub fn set_slot(&mut self, slot: u32) -> &mut Self {
    debug_assert!(slot < (MAX_SLOT as u32));
    self.0 &= !((MAX_SLOT as u32) - 1);
    self.0 |= slot;
    self
  }
}

impl fmt::Debug for Rid {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("Rid").field("page", &self.page()).field("slot", &self.slot()).finish()
  }
}

fn _ck() { assert_eq_size!(Rid, u32); }