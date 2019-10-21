use std::ptr::NonNull;

use common::*;
use physics::*;
use crate::Db;

impl Db {
  pub unsafe fn record_iter(&mut self, tp: &TablePage) -> RecordIter {
    RecordIter { db: self.pr(), page: tp.first, slot: 0, slot_size: tp.size, cap: tp.cap }
  }
}

pub struct RecordIter<'a> {
  db: &'a mut Db,
  page: u32,
  slot: u16,
  slot_size: u16,
  cap: u16,
}

impl Iterator for RecordIter<'_> {
  type Item = (NonNull<u8>, Rid);

  fn next(&mut self) -> Option<Self::Item> {
    unsafe {
      loop {
        if self.page == !0 { return None; }
        // now self.page must be a valid data page id
        let dp = self.db.get_page::<DataPage>(self.page);
        for i in self.slot as usize..self.cap as usize {
          if bsget(dp.used.as_ptr(), i) {
            self.slot = i as u16 + 1;
            let data = dp.data.as_mut_ptr().add(i * self.slot_size as usize);
            let rid = Rid::new(self.page, i as u32);
            return Some((NonNull::new_unchecked(data), rid));
          }
        }
        self.page = dp.next;
        self.slot = 0;
      }
    }
  }
}