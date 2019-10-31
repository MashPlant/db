use common::*;
use physics::*;
use crate::Db;

impl Db {
  pub unsafe fn get_lob(&mut self, id: u32) -> *mut u8 {
    (self.lob_mmap.as_mut_ptr() as *mut FreeLobSlot).add(id as usize) as *mut u8
  }

  // return (lob id, actual bytes allocated, start addr of lob), lob id can be used for get & dealloc
  pub unsafe fn alloc_lob(&mut self, count: u32) -> (u32, u32, *mut u8) {
    let count = ((count + LOB_SLOT_SIZE as u32 - 1) / LOB_SLOT_SIZE as u32).max(1); // .max(1) to avoid alloc 0 uses the nil node
    let base = self.lob_mmap.as_mut_ptr() as *mut FreeLobSlot;
    let mut x = base.r();
    while x.count < count {
      if x.next == 0 { break; } else { x = base.add(x.next as usize).r(); }
    }
    if x.count >= count {
      if x.count > count { self.shift_lob_link(x, count); } else {
        let (prev, next) = (x.prev, x.next);
        base.add(prev as usize).r().next = next;
        base.add(next as usize).r().prev = prev;
      }
      (x.p().offset_from(base) as u32, count * 32, x.p() as *mut u8)
    } else { // get out of `while` because of `break`
      let id = (self.lob_slots, self.lob_slots += count).0;
      self.lob_file.set_len(self.lob_slots as u64 * LOB_SLOT_SIZE as u64).expect("failed to allocate lob slot. the database may already be in an invalid state.");
      (id, count * 32, base.add(id as usize) as *mut u8)
    }
  }

  pub unsafe fn dealloc_lob(&mut self, id: u32, count: u32) {
    debug_assert!(count != 0 && count % LOB_SLOT_SIZE as u32 == 0);
    let count = count / LOB_SLOT_SIZE as u32;
    let base = self.lob_mmap.as_mut_ptr() as *mut FreeLobSlot;
    let (mut x_id, mut x) = (0, base.r());
    loop {
      if x_id + x.count == id {
        return x.count += count;
      } else if id + count == x_id {
        return self.shift_lob_link(x, !count + 1); // !count + 1 == -count
      } else {
        x_id = x.next;
        if x_id == 0 { break; } else { x = base.add(x_id as usize).r(); }
      }
    }
    // fails to extend any existing nodes, add to back
    let nil = base.r();
    let prev = base.add(nil.prev as usize).r();
    let new = base.add(id as usize).r();
    (prev.next = id, new.prev = nil.prev);
    (new.next = 0, nil.prev = id);
    new.count = count;
  }

  // `shift as i32` can be negative
  unsafe fn shift_lob_link(&mut self, x: &FreeLobSlot, shift: u32) {
    let base = self.lob_mmap.as_mut_ptr() as *mut FreeLobSlot;
    let (prev, next, new_x_id) = (x.prev, x.next, (x.p().offset_from(base) as u32).wrapping_add(shift));
    let new_x = base.add(new_x_id as usize).r();
    (base.add(prev as usize).r().next = new_x_id, new_x.prev = prev);
    (base.add(next as usize).r().prev = new_x_id, new_x.next = next);
    new_x.count = x.count.wrapping_sub(shift);
  }
}