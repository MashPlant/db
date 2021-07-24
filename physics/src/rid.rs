use std::{fmt, num::NonZeroU32};
use common::{MAX_SLOT, LOG_MAX_SLOT};

// (32 - LOG_MAX_SLOT) bits for page, LOG_MAX_SLOT bits for slot
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct Rid(NonZeroU32); // page 0 cannot be used in rid, so rid cannot be 0

impl Rid {
  pub unsafe fn new(page: u32, slot: u32) -> Rid { Rid(NonZeroU32::new_unchecked((page << LOG_MAX_SLOT) | slot)) }
  pub fn page(self) -> u32 { self.0.get() >> LOG_MAX_SLOT }
  pub fn slot(self) -> u32 { self.0.get() & ((MAX_SLOT as u32) - 1) }
}

impl fmt::Debug for Rid {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("Rid").field("page", &self.page()).field("slot", &self.slot()).finish()
  }
}

#[cfg_attr(tarpaulin, ignore)]
fn _ck() {
  assert_eq_size!(Rid, u32);
  assert_eq_size!(Rid, Option<Rid>); // thanks to NonZeroU32
}