#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(ptr_offset_from)]

use std::{ptr::{self, NonNull}, marker::PhantomData, cmp::Ordering};

use common::*;
use db::Db;
use physics::*;
use crate::cmp::*;

pub mod cmp;
pub mod iter;
pub mod alter;

pub use alter::*;

// using both lifetime parameter and const parameter will cause my rustc (1.40.0-nightly) to ICE, so just use pointer here
pub struct Index<const T: BareTy> {
  db: *mut Db,
  root: u32,
  table: u32,
  col: u32,
  _p: PhantomData<Cmp<{ T }>>,
}

impl<const T: BareTy> Index<{ T }> {
  pub unsafe fn new(db: &mut Db, table: u32, col: u32) -> Index<{ T }> {
    let tp = db.get_page::<TablePage>(table);
    let root = tp.cols.get_unchecked_mut(col as usize).index;
    Index { db, root, table, col, _p: PhantomData }
  }

  unsafe fn db<'a>(&mut self) -> &'a mut Db { self.db.r() }
  unsafe fn rid_off(&self) -> usize { self.db.r().get_page::<IndexPage>(self.root).rid_off as usize }

  // caller guarantee data_rid doesn't exist in tree
  pub unsafe fn insert(&mut self, data: *const u8, rid: Rid) {
    let data_rid = self.make_data_rid(data, rid);
    if let Some((overflow, split_page)) = self.do_insert(self.root, data_rid.ptr) {
      let (new_id, new_ip) = self.db().alloc_page::<IndexPage>();
      let old = self.db().get_page::<IndexPage>(self.root);
      (new_ip.next = !0, new_ip.count = 2, new_ip.leaf = false, new_ip.rid_off = old.rid_off);
      new_ip.cap = MAX_INDEX_BYTES as u16 / new_ip.slot_size();
      let p = new_ip.data.as_mut_ptr();
      let (slot_size, key_size) = (new_ip.slot_size() as usize, new_ip.key_size() as usize);
      p.copy_from_nonoverlapping(old.data.as_ptr(), key_size); // data_rid0
      *(p.add(key_size) as *mut u32) = self.root; // child0
      p.add(slot_size).copy_from_nonoverlapping(overflow.as_ptr(), key_size); // data_rid1
      *(p.add(slot_size + key_size) as *mut u32) = split_page; // child1
      self.make_root(new_id);
    }
  }

  // return Some((ptr to the first data_rid in new page, new page id)) if overflow happens
  // using NonNull is to optimize Option's space
  unsafe fn do_insert(&mut self, page: u32, x: *const u8) -> Option<(NonNull<u8>, u32)> {
    self.debug_check(page);
    let ip = self.db().get_page::<IndexPage>(page);
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
      insert!(upper_bound::<{ T }>(ip, x), x);
    } else {
      let ub = upper_bound::<{ T }>(ip, x);
      let pos = if ub == 0 {
        (at!(0).copy_from_nonoverlapping(x, key_size), 0).1 // update min key
      } else { ub - 1 }; // insert before `lb`
      if let Some((overflow, split_page)) = self.do_insert(at_ch!(pos), x) {
        // `split_page` comes from the mid of the splitted child (`at_ch!(pos)`), it can only be at `at_ch!(pos + 1)`
        insert!(pos + 1, overflow.as_ptr());
        at_ch!(pos + 1) = split_page;
      }
    }
    if ip.count == ip.cap {
      let (sp_id, sp_ip) = self.db().alloc_page::<IndexPage>();
      (sp_ip.next = ip.next, ip.next = sp_id);
      // split ceiling half to new page, which keeps the mid key
      (sp_ip.count = ip.count - ip.count / 2, ip.count /= 2);
      (sp_ip.leaf = ip.leaf, sp_ip.rid_off = ip.rid_off, sp_ip.cap = ip.cap);
      sp_ip.data.as_mut_ptr().copy_from_nonoverlapping(at!(ip.count as usize), sp_ip.count as usize * slot_size);
      Some((NonNull::new_unchecked(sp_ip.data.as_mut_ptr()), sp_id))
    } else { None }
  }

  // caller guarantee data_rid exists in tree
  pub unsafe fn delete(&mut self, data: *const u8, rid: Rid) {
    self.do_delete(self.root, self.make_data_rid(data, rid).ptr);
  }

  // return (pointer to the min key in page, does page need merge (count < cap / 2))
  unsafe fn do_delete(&mut self, page: u32, x: *const u8) -> (*const u8, bool) {
    self.debug_check(page);
    let ip = self.db().get_page::<IndexPage>(page);
    let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
    macro_rules! at { ($pos: expr) => { ip.data.as_mut_ptr().add($pos * slot_size) }; }
    macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
    macro_rules! remove {
      ($pos: expr) => {
        ip.count -= 1;
        at!($pos).copy_from(at!($pos + 1), (ip.count as usize - $pos) * slot_size);
      };
    }
    if ip.leaf {
      let pos = upper_bound::<{ T }>(ip, x) - 1;
      debug_assert_eq!(Cmp::<{ T }>::cmp_full(x, at!(pos), self.rid_off()), Ordering::Equal);
      remove!(pos);
    } else {
      let pos = upper_bound::<{ T }>(ip, x).max(1) - 1;
      let (new_min, need_merge) = self.do_delete(at_ch!(pos), x);
      at!(pos).copy_from_nonoverlapping(new_min, key_size); // update dup key
      if need_merge {
        if ip.count == 1 {
          debug_assert!(page == self.root); // only root can have so few slots
          self.db().dealloc_page(page);
          self.make_root(at_ch!(0));
        } else {
          let l = if pos + 1 < ip.count as usize { pos } else { pos - 1 };
          let (lid, rid) = (at_ch!(l), at_ch!(l + 1));
          let (lp, rp) = (self.db().get_page::<IndexPage>(lid), self.db().get_page::<IndexPage>(rid));
          debug_assert_ne!(lid, rid);
          debug_assert_eq!(lp.cap, rp.cap); // but they mey not be equal to ip.cap
          debug_assert_eq!(lp.slot_size(), rp.slot_size()); // but they mey not be equal to ip.slot_size()
          let slot_size = lp.slot_size() as usize; // `key_size` is the same as `ip`'s
          if lp.count + rp.count < lp.cap { // do merge, merge r to l
            debug_assert!(lp.count + rp.count >= lp.cap / 2);
            lp.next = rp.next;
            lp.data.as_mut_ptr().add(lp.count as usize * slot_size).copy_from_nonoverlapping(rp.data.as_ptr(), rp.count as usize * slot_size);
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
    let tp = self.db().get_page::<TablePage>(self.table);
    tp.cols.get_unchecked_mut(self.col as usize).index = new_id;
  }

  unsafe fn make_data_rid(&self, data: *const u8, rid: Rid) -> Align4U8 {
    let data_rid = Align4U8::new(self.rid_off() + 4);
    data_rid.ptr.copy_from_nonoverlapping(data, self.rid_off());
    *(data_rid.ptr.add(self.rid_off()) as *mut Rid) = rid;
    data_rid
  }

  unsafe fn debug_check(&self, page: u32) {
    if cfg!(debug_assertions) { // ensure compiler can optimize this out
      let ip = self.pr().db().get_page::<IndexPage>(page);
      let slot_size = ip.slot_size() as usize;
      macro_rules! at { ($pos: expr) => { ip.data.as_mut_ptr().add($pos * slot_size) }; }
      // previously the relationship between `cap` and `slot_size` is checked here (in the commented line)
      // but it is now removed because we want to modify the `cap` in tests without modifying `slot_size`
      // assert_eq!(ip.cap, MAX_INDEX_BYTES as u16 / ip.slot_size());
      assert!(ip.count < ip.cap); // cannot have count == cap, the code depends on it
      assert!(page == self.root || ip.cap / 2 <= ip.count);
      assert_eq!(ip.rid_off as usize, self.rid_off());
      for i in 1..ip.count as usize {
        assert_eq!(Cmp::<{ T }>::cmp_full(at!(i - 1), at!(i), self.rid_off()), Ordering::Less);
      }
    }
  }

  pub unsafe fn debug_check_all(&self) {
    if cfg!(debug_assertions) {
      let tp = self.pr().db().get_page::<TablePage>(self.table);
      assert_eq!(tp.cols[self.col as usize].index, self.root);
      self.dfs(self.root, ptr::null(), ptr::null());
    }
  }

  unsafe fn dfs(&self, page: u32, lb: *const u8, ub: *const u8) {
    self.debug_check(page);
    let ip = self.pr().db().get_page::<IndexPage>(page);
    let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
    macro_rules! at { ($pos: expr) => { ip.data.as_mut_ptr().add($pos * slot_size) }; }
    macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
    if !lb.is_null() {
      // the min key must be the dup key
      assert_eq!(Cmp::<{ T }>::cmp_full(lb, at!(0), self.rid_off()), Ordering::Equal);
    }
    if !ub.is_null() {
      assert_eq!(Cmp::<{ T }>::cmp_full(at!(ip.count as usize - 1), ub, self.rid_off()), Ordering::Less);
    }
    if !ip.leaf {
      for i in 0..ip.count as usize {
        let ub = if i + 1 == ip.count as usize { ub } else { at!(i + 1) };
        self.dfs(at_ch!(i), at!(i), ub);
      }
    }
  }
}

#[macro_use]
pub(crate) mod macros {
  // handle all kinds of Index with regard to different types
  #[macro_export]
  macro_rules! handle_all {
    ($ty: expr, $handle: ident) => {
      match $ty { Bool => $handle!(Bool), Int => $handle!(Int), Float => $handle!(Float), VarChar => $handle!(VarChar), Date => $handle!(Date) }
    };
  }
}