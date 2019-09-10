pub struct IndexPage {
  // !0 for invalid
  pub next: u32,
  pub count: u16,
  pub leaf: bool,
  pub _rsv: u8,
  // actually these 2 fields are not so necessary, because when an index is used, size information are always available
  // and they can be calculated; but placing them here brings some convenience
  pub rid_off: u16,
  pub cap: u16,
  // array of (data, rid, child) for inner, (data, rid) for leaf
  // notice that data_rid are always consecutive
  pub data: [u8; MAX_INDEX_BYTES as usize],
}

pub const MAX_INDEX_BYTES: u32 = 8180;

impl IndexPage {
  #[inline]
  pub fn init(&mut self, leaf: bool, ty_size: u16) {
    self.next = !0;
    self.count = 0;
    self.leaf = leaf;
    self.rid_off = (ty_size + 3) & !3;
    self.cap = MAX_INDEX_BYTES as u16 / self.slot_size();
  }
  // `key` contains both data and rid
  #[inline]
  pub fn key_size(&self) -> u16 { self.rid_off + 4 }
  #[inline]
  pub fn slot_size(&self) -> u16 { self.key_size() + if self.leaf { 0 } else { 4 } }
}

fn _ck() { const_assert_eq!(std::mem::size_of::<IndexPage>(), common::PAGE_SIZE); }