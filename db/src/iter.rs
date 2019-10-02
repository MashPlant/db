use std::ptr::NonNull;

use common::*;
use physics::*;
use crate::Db;

impl Db {
  pub unsafe fn record_iter(&mut self, tp: &TablePage) -> RecordIter {
    let tp_id = (tp as *const TablePage).offset_from(self.get_page(0)) as u32;
    RecordIter { db: self.pr(), tp_id, page: tp.next, slot: 0, slot_size: tp.size }
  }
}

pub struct RecordIter<'a> {
  db: &'a mut Db,
  // nil node in the linked list, also the page if of TablePage
  tp_id: u32,
  page: u32,
  slot: u16,
  slot_size: u16,
}

impl Iterator for RecordIter<'_> {
  type Item = (NonNull<u8>, Rid);

  fn next(&mut self) -> Option<Self::Item> {
    unsafe {
      loop {
        if self.page == self.tp_id { return None; } // reach end of linked list
        // now self.page must be a valid data page id
        let dp = self.db.get_page::<DataPage>(self.page);
        let cap = self.db.get_page::<TablePage>(self.tp_id).cap;
        for i in self.slot as usize..cap as usize {
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