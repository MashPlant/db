use common::*;
use physics::*;
use crate::Db;

impl Db {
  pub unsafe fn record_iter<'a>(&mut self, tp: &TablePage) -> RecordIter<'a> {
    RecordIter { db: self.pr(), page: tp.first, slot: 0, size: tp.size, cap: tp.cap }
  }
}

pub struct RecordIter<'a> {
  db: &'a mut Db,
  page: u32,
  slot: u16,
  size: u16,
  cap: u16,
}

impl Iterator for RecordIter<'_> {
  type Item = (*mut u8, Rid);

  fn next(&mut self) -> Option<Self::Item> {
    unsafe {
      loop {
        if self.page == !0 { return None; }
        // now self.page must be a valid data page id
        let dp = self.db.get_page::<DataPage>(self.page);
        for i in self.slot as usize..self.cap as usize {
          if bsget(dp.used.as_ptr(), i) {
            self.slot = i as u16 + 1;
            let data = dp.data.as_mut_ptr().add(i * self.size as usize);
            return Some((data, Rid::new(self.page, i as u32)));
          }
        }
        self.page = dp.next;
        self.slot = 0;
      }
    }
  }
}