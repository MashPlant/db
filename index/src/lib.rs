#![allow(incomplete_features)]
#![feature(const_generics)]

use std::{ptr::NonNull, marker::PhantomData, cmp::Ordering};

use common::*;
use db::Db;
use physics::*;
use crate::cmp::*;

pub mod cmp;
pub mod iter;
pub mod create;

pub use create::create;

// using both lifetime parameter and const parameter will cause my rustc(1.38.0-nightly) to ICE
// so just use pointer here
pub struct Index<const T: BareTy> {
  db: NonNull<Db>,
  root: u32,
  // col points to (table_page in db, col_slot in table) in TablePage
  col: Rid,
  rid_off: u16,
  _p: PhantomData<Cmp<{ T }>>,
}

impl<const T: BareTy> Index<{ T }> {
  pub unsafe fn new(db: &mut Db, col: Rid) -> Index<{ T }> {
    let tp = db.get_page::<TablePage>(col.page() as usize);
    let root = tp.cols.get_unchecked_mut(col.slot() as usize).index;
    let rid_off = db.get_page::<IndexPage>(root as usize).rid_off;
    Index { db: NonNull::new_unchecked(db), root, col, rid_off, _p: PhantomData }
  }

  unsafe fn db<'a>(&mut self) -> &'a mut Db { self.db.as_ptr().r() }

  pub unsafe fn insert(&mut self, data: *const u8, rid: Rid) {
    let data_rid = self.make_data_rid(data, rid);
    if let Some((overflow, split_page)) = self.do_insert(self.root as usize, data_rid.ptr) {
      let (new_id, new_ip) = self.db().alloc_page::<IndexPage>();
      let old = self.db().get_page::<IndexPage>(self.root as usize);
      (new_ip.next = !0, new_ip.count = 2, new_ip.leaf = false, new_ip.rid_off = old.rid_off);
      new_ip.cap = MAX_INDEX_BYTES as u16 / new_ip.slot_size();
      let p = new_ip.data.as_mut_ptr();
      let (slot_size, key_size) = (new_ip.slot_size() as usize, new_ip.key_size() as usize);
      p.copy_from_nonoverlapping(old.data.as_ptr(), key_size); // data_rid0
      *(p.add(key_size) as *mut u32) = self.root; // child0
      p.add(slot_size).copy_from_nonoverlapping(overflow.as_ptr(), key_size); // data_rid1
      *(p.add(slot_size + key_size) as *mut u32) = split_page; // child1
      self.make_root(new_id as u32);
    }
  }

  // return Some((ptr to the first data_rid in new page, new page id)) if overflow happens
  // using NonNull is to optimize Option's space
  unsafe fn do_insert(&mut self, page: usize, x: *const u8) -> Option<(NonNull<u8>, u32)> {
    let ip = self.db().get_page::<IndexPage>(page);
    self.debug_check(page, ip);
    let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
    macro_rules! at { ($pos: expr) => { ip.data.as_mut_ptr().add($pos * slot_size) }; }
    macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
    macro_rules! insert {
      ($pos: expr, $x: expr) => {
        at!($pos + 1).copy_from(at!($pos), (ip.count as usize - $pos) * slot_size);
        at!($pos).copy_from_nonoverlapping($x, key_size);
        ip.count += 1;
      };
    }
    if ip.leaf {
      let lb = lower_bound::<{ T }>(ip, x);
      debug_assert_ne!(Cmp::<{ T }>::cmp_full(x, at!(lb), self.rid_off as usize), Ordering::Equal);
      insert!(lb, x);
    } else {
      let ub = upper_bound::<{ T }>(ip, x);
      let pos = if ub == 0 && Cmp::<{ T }>::cmp_full(x, at!(0), self.rid_off as usize) == Ordering::Less {
        (at!(0).copy_from_nonoverlapping(x, key_size), 0).1 // update min key
      } else { ub - 1 }; // insert before ub
      if let Some((overflow, split_page)) = self.do_insert(at_ch!(pos) as usize, x) {
        let lb = lower_bound::<{ T }>(ip, overflow.as_ptr());
        insert!(lb, overflow.as_ptr());
        at_ch!(lb) = split_page;
      }
    }
    if ip.count == ip.cap {
      let (sp_id, sp_ip) = self.db().alloc_page::<IndexPage>();
      (sp_ip.next = ip.next, ip.next = sp_id as u32);
      // split ceiling half to new page, which keeps the mid key
      (sp_ip.count = ip.count - ip.count / 2, ip.count /= 2);
      (sp_ip.leaf = ip.leaf, sp_ip.rid_off = ip.rid_off, sp_ip.cap = ip.cap);
      sp_ip.data.as_mut_ptr().copy_from_nonoverlapping(at!(ip.count as usize), sp_ip.count as usize * slot_size);
      Some((NonNull::new_unchecked(sp_ip.data.as_mut_ptr()), sp_id as u32))
    } else { None }
  }

  pub unsafe fn delete(&mut self, data: *const u8, rid: Rid) {
    self.do_delete(self.root as usize, self.make_data_rid(data, rid).ptr);
  }

  unsafe fn do_delete(&mut self, page: usize, x: *const u8) -> (*const u8, bool) {
    let ip = self.db().get_page::<IndexPage>(page);
    self.debug_check(page, ip);
    let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
    macro_rules! at { ($pos: expr) => { ip.data.as_mut_ptr().add($pos * slot_size) }; }
    macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
    macro_rules! remove {
      ($pos: expr) => {
        ip.count -= 1;
        at!($pos).copy_from(at!($pos + 1), (ip.count as usize - $pos - 1) * slot_size);
      };
    }
    if ip.leaf {
      let lb = lower_bound::<{ T }>(ip, x);
      debug_assert_eq!(Cmp::<{ T }>::cmp_full(x, at!(lb), self.rid_off as usize), Ordering::Equal);
      remove!(lb);
    } else {
      let pos = upper_bound::<{ T }>(ip, x).max(1) - 1;
      let (new_min, need_merge) = self.do_delete(at_ch!(pos) as usize, x);
      at!(pos).copy_from_nonoverlapping(new_min, key_size); // update dup key
      if need_merge {
        if ip.count == 1 {
          debug_assert!(page == self.root as usize); // only root can have so few slots
          self.db().dealloc_page(page);
          self.make_root(at_ch!(0));
        } else {
          let l = if pos + 1 < ip.count as usize { pos } else { pos - 1 };
          let (lid, rid) = (at_ch!(l) as usize, at_ch!(l + 1) as usize);
          let (lp, rp) = (self.db().get_page::<IndexPage>(lid), self.db().get_page::<IndexPage>(rid));
          debug_assert_ne!(lid, rid);
          debug_assert_eq!(lp.cap, rp.cap); // but they mey not be equal to ip.cap
          if lp.count + rp.count < lp.cap { // do merge
            debug_assert!(lp.count + rp.count >= lp.cap / 2);
            if rp.next == 0 {
              lp.next = rp.next;
            }
            lp.next = rp.next;
            lp.data.as_mut_ptr().add(lp.count as usize * slot_size)
              .copy_from_nonoverlapping(rp.data.as_ptr(), rp.count as usize * slot_size);
            lp.count += rp.count;
            remove!(l + 1); // r is overwritten
            self.db().dealloc_page(rid);
          } else { // do transfer, make each of them have same number of keys
            let tot = lp.count + rp.count;
            if lp.count < tot / 2 {
              let diff = (tot / 2 - lp.count) as usize * slot_size;
              lp.data.as_mut_ptr().add(lp.count as usize * slot_size).copy_from_nonoverlapping(rp.data.as_ptr(), diff);
              rp.data.as_mut_ptr().copy_from(rp.data.as_ptr().add(diff), rp.count as usize * slot_size - diff);
            } else {
              let diff = (lp.count - tot / 2) as usize * slot_size;
              rp.data.as_mut_ptr().add(diff).copy_from(rp.data.as_ptr(), rp.count as usize * slot_size);
              rp.data.as_mut_ptr().copy_from_nonoverlapping(lp.data.as_ptr().add((tot / 2) as usize * slot_size), diff);
            }
            (lp.count = tot / 2, rp.count = tot - tot / 2);
            at!(l + 1).copy_from_nonoverlapping(rp.data.as_ptr(), key_size); // update dup key
          }
        }
      }
    }
    (at!(0), ip.count < ip.cap / 2)
  }

  unsafe fn make_root(&mut self, new_id: u32) {
    self.root = new_id;
    let tp = self.db().get_page::<TablePage>(self.col.page() as usize);
    tp.cols.get_unchecked_mut(self.col.slot() as usize).index = new_id;
  }

  unsafe fn make_data_rid(&self, data: *const u8, rid: Rid) -> Align4U8 {
    let data_rid = Align4U8::new(self.rid_off as usize + 4);
    data_rid.ptr.copy_from_nonoverlapping(data, self.rid_off as usize);
    *(data_rid.ptr.add(self.rid_off as usize) as *mut Rid) = rid;
    data_rid
  }

  fn debug_check(&self, page: usize, ip: &IndexPage) {
    debug_assert_eq!(ip.cap, MAX_INDEX_BYTES as u16 / ip.slot_size());
    debug_assert!(ip.count < ip.cap); // cannot have count == cap, the code depends on it
    debug_assert!(page == self.root as usize || ip.cap / 2 <= ip.count);
    debug_assert!(ip.leaf || ip.next == !0);
    debug_assert_eq!(ip.rid_off, self.rid_off);
  }
}

#[macro_use]
pub(crate) mod macros {
  // handle all kinds of Index with regard to different types
  #[macro_export]
  macro_rules! handle_all {
    ($ty: expr, $handle: ident) => {
      match $ty { Int => $handle!(Int), Bool => $handle!(Bool), Float => $handle!(Float), Char => $handle!(Char), VarChar => $handle!(VarChar), Date => $handle!(Date) }
    };
  }
}