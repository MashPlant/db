use std::mem;

use common::*;
use db::Db;
use physics::{IndexPage, Rid};
use crate::{Index, cmp};

pub struct IndexIter<'a> {
  db: &'a mut Db,
  page: u32,
  slot: u16,
}

impl IndexIter<'_> {
  pub unsafe fn next(&mut self) -> Option<Rid> {
    let mut ip = self.db.get_page::<IndexPage>(self.page);
    if self.slot == ip.count {
      if ip.next == !0 { return None; }
      self.page = ip.next;
      self.slot = 0;
      ip = self.db.get_page::<IndexPage>(self.page);
    }
    let slot = (self.slot, self.slot += 1).0;
    let data_rid = ip.data.as_mut_ptr().add((slot * ip.slot_size()) as usize);
    let rid = *(data_rid.add(ip.rid_off as usize) as *const Rid);
    Some(rid)
  }
}

impl PartialEq for IndexIter<'_> {
  fn eq(&self, other: &Self) -> bool { self.page == other.page && self.slot == other.slot }
}

impl<const T: BareTy> Index<{ T }> {
  pub unsafe fn iter<'a>(&mut self) -> IndexIter<'a> {
    let mut page = self.root;
    loop {
      let ip = self.db().get_page::<IndexPage>(page);
      if ip.leaf { break IndexIter { db: self.db(), page, slot: 0 }; }
      page = *(ip.data.as_mut_ptr().add(ip.key_size() as usize) as *mut u32);
    }
  }

  pub unsafe fn lower_bound<'a>(&mut self, data: *const u8) -> IndexIter<'a> {
    // 00..00 is the smallest, but this will trigger a warning (because Rid is marked as non-zero)
    // so use 00..01, it is also small enough
    let data_rid = self.make_data_rid(data, mem::transmute(1));
    let (page, slot) = self.do_upper_bound(data_rid.ptr);
    IndexIter { db: self.db(), page, slot }
  }

  pub unsafe fn upper_bound<'a>(&mut self, data: *const u8) -> IndexIter<'a> {
    // rid = 11..11, which is the biggest
    let data_rid = self.make_data_rid(data, mem::transmute(!0));
    let (page, slot) = self.do_upper_bound(data_rid.ptr);
    IndexIter { db: self.db(), page, slot }
  }

  pub unsafe fn contains(&self, data: *const u8) -> bool {
    self.pr().lower_bound(data) != self.pr().upper_bound(data)
  }

  unsafe fn do_upper_bound(&mut self, data_rid: *const u8) -> (u32, u16) {
    let mut page = self.root;
    loop {
      self.debug_check(page);
      let ip = self.db().get_page::<IndexPage>(page);
      let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
      macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
      if ip.leaf { break (page, cmp::upper_bound::<{ T }>(ip, data_rid) as u16); }
      let pos = cmp::upper_bound::<{ T }>(ip, data_rid).max(1) - 1;
      page = at_ch!(pos);
    }
  }
}