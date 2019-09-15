use std::ptr::NonNull;

use common::*;
use physics::*;
use crate::Db;

impl Db {
  pub unsafe fn record_iter(&mut self, tp: &TablePage) -> RecordIter {
    RecordIter { db: self.pr(), head: self.id_of(tp) as u32, page: tp.next, slot: 0, slot_size: tp.size }
  }
}

pub struct RecordIter<'a> {
  db: &'a mut Db,
  // nil node in the linked list, also the page if of TablePage
  head: u32,
  page: u32,
  slot: u16,
  slot_size: u16,
}

impl Iterator for RecordIter<'_> {
  type Item = (NonNull<u8>, Rid);

  fn next(&mut self) -> Option<Self::Item> {
    unsafe {
      loop {
        let dp = self.db.get_page::<DataPage>(self.page as usize);
        for i in self.slot as usize..MAX_SLOT_BS {
          if ((*dp.used.get_unchecked(i / 32) >> (i as u32 % 32)) & 1) != 0 {
            self.slot = i as u16 + 1;
            let data = dp.data.as_mut_ptr().add(i * self.slot_size as usize);
            let rid = Rid::new(self.page, i as u32);
            return Some((NonNull::new_unchecked(data), rid));
          }
        }
        if dp.next != self.head {
          self.page = dp.next;
          self.slot = 0;
        } else { return None; }
      }
    }
  }
}