use std::{ptr::NonNull, mem};

use common::*;
use db::Db;
use physics::IndexPage;
use crate::{Index, cmp};

pub struct IndexIter {
  db: *mut Db,
  page: u32,
  slot: u16,
}

impl IndexIter {
  pub unsafe fn next(&mut self) -> Option<NonNull<u8>> {
    let mut ip = self.db.r().get_page::<IndexPage>(self.page as usize);
    if self.slot == ip.count {
      if ip.next == !0 { return None; }
      self.page = ip.next;
      self.slot = 0;
      ip = self.db.r().get_page::<IndexPage>(self.page as usize);
    }
    let slot = (self.slot, self.slot += 1).0;
    Some(NonNull::new_unchecked(ip.data.as_mut_ptr().add((slot * ip.slot_size()) as usize)))
  }
}

impl PartialEq for IndexIter {
  fn eq(&self, other: &Self) -> bool {
    debug_assert_eq!(self.db, other.db);
    self.page == other.page && self.slot == other.slot
  }
}

impl<const T: BareTy> Index<{ T }> {
  pub unsafe fn iter(&mut self) -> IndexIter {
    let mut page = self.root as usize;
    loop {
      let ip = self.db().get_page::<IndexPage>(page);
      if ip.leaf {
        break IndexIter { db: self.db(), page: page as u32, slot: 0 };
      }
      page = *(ip.data.as_mut_ptr().add(ip.key_size() as usize) as *mut u32) as usize;
    }
  }

  pub unsafe fn lower_bound(&mut self, data: *const u8) -> IndexIter {
    // rid = 00..00, which is the smallest
    let data_rid = self.make_data_rid(data, mem::transmute(0));
    let (page, slot) = self.do_lower_bound(data_rid.ptr);
    IndexIter { db: self.db(), page, slot }
  }

  pub unsafe fn upper_bound(&mut self, data: *const u8) -> IndexIter {
    // rid = 11..11, which is the biggest
    let data_rid = self.make_data_rid(data, mem::transmute(!0));
    let (page, slot) = self.do_lower_bound(data_rid.ptr);
    IndexIter { db: self.db(), page, slot }
  }

  #[inline]
  pub unsafe fn contains(&self, x: *const u8) -> bool {
    self.p().r().lower_bound(x) != self.p().r().upper_bound(x)
  }

  unsafe fn do_lower_bound(&mut self, x: *const u8) -> (u32, u16) {
    let mut page = self.root as usize;
    loop {
      let ip = self.db().get_page::<IndexPage>(page);
      self.debug_check(page, ip);
      let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
      macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
      if ip.leaf {
        break (page as u32, cmp::lower_bound::<{ T }>(ip, x) as u16);
      }
      let pos = cmp::upper_bound::<{ T }>(ip, x).max(1) - 1;
      page = at_ch!(pos) as usize;
    }
  }
}