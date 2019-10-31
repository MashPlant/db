use std::mem::size_of;

#[repr(C)]
pub struct DataPage {
  // !0 for none
  pub next: u32,
  // !0 for none
  pub next_free: u32,
  pub count: u16,
  pub _rsv: [u8; 2],
  pub used: [u32; common::MAX_SLOT_BS],
  pub data: [u8; common::MAX_DATA_BYTE],
}

impl DataPage {
  pub unsafe fn init(&mut self, next: u32) {
    self.next = next;
    self.next_free = !0;
    self.count = 0;
    self.used.as_mut_ptr().write_bytes(0, common::MAX_SLOT_BS);
  }
}

// for simplicity, one check list always use one page, if size exceeds the limit, just reject it
#[repr(C)]
pub struct CheckPage {
  pub count: u16,
  pub _rsv: [u8; 2],
  pub data: [u8; MAX_CHECK_BYTES],
}

pub const MAX_CHECK_BYTES: usize = 8188;

// a blob slot can either be a FreeBlobSlot, or a [u8; 32]
#[repr(C)]
pub struct FreeLobSlot {
  // circular linked list, 0 is the nil node
  pub prev: u32,
  pub next: u32,
  pub count: u32,
  // useless, just to make FreeBlobSlot and [u8; 32] have the same size (though this is not necessary, either)
  pub _rsv: [u8; 20],
}

pub const LOB_SLOT_SIZE: usize = 32;

impl FreeLobSlot {
  pub fn init_nil(&mut self) { (self.prev = 0, self.next = 0, self.count = 0); }
}

// this is how varchar exists in DataPage
#[repr(C)]
pub struct VarcharSlot {
  pub lob_id: u32,
  pub len: u16,
  // `cap` is used to deallocate
  pub cap: u16,
}

#[cfg_attr(tarpaulin, skip)]
fn _ck() {
  const_assert_eq!(size_of::<DataPage>(), common::PAGE_SIZE);
  const_assert_eq!(size_of::<CheckPage>(), common::PAGE_SIZE);
  const_assert_eq!(size_of::<FreeLobSlot>(), LOB_SLOT_SIZE);
  const_assert_eq!(size_of::<VarcharSlot>(), common::VARCHAR_SLOT_SIZE);
}