use std::{fmt, num::NonZeroU32};
use common::{MAX_PAGE, MAX_SLOT, LOG_MAX_SLOT};

// (32 - LOG_MAX_SLOT) bits for page, LOG_MAX_SLOT bits for slot
// enough to locate both a record in DataPage and a col_info in TablePage
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct Rid(NonZeroU32); // page 0 cannot be used in rid, so rid cannot be 0

impl Rid {
  pub fn new(page: u32, slot: u32) -> Rid {
    debug_assert!(page != 0);
    debug_assert!(page < (MAX_PAGE as u32));
    debug_assert!(slot < (MAX_SLOT as u32));
    unsafe { Rid(NonZeroU32::new_unchecked((page << LOG_MAX_SLOT) | slot)) }
  }

  pub fn page(self) -> u32 { self.0.get() >> LOG_MAX_SLOT }
  pub fn slot(self) -> u32 { self.0.get() & ((MAX_SLOT as u32) - 1) }
}

impl fmt::Debug for Rid {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("Rid").field("page", &self.page()).field("slot", &self.slot()).finish()
  }
}

#[cfg_attr(tarpaulin, skip)]
fn _ck() {
  assert_eq_size!(Rid, u32);
  assert_eq_size!(Rid, Option<Rid>); // thanks to NonZeroU32
}