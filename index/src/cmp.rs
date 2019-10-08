use std::cmp::Ordering;
use chrono::NaiveDate;
use unchecked_unwrap::UncheckedUnwrap;

use common::*;
use physics::*;

pub struct Cmp<const T: BareTy>;

impl<const T: BareTy> Cmp<{ T }> {
  pub unsafe fn cmp(l: *const u8, r: *const u8) -> Ordering {
    use BareTy::*;
    match T { // should be optimized out
      Bool => (*(l as *const bool)).cmp(&*(r as *const bool)),
      Int => (*(l as *const i32)).cmp(&*(r as *const i32)),
      // it is safe because lexer only allow float like xxx.xxx comes in, and they are all comparable
      Float => (*(l as *const f32)).partial_cmp(&*(r as *const f32)).unchecked_unwrap(),
      Date => (*(l as *const NaiveDate)).cmp(&*(r as *const NaiveDate)),
      VarChar => str_from_db(l).cmp(str_from_db(r)),
    }
  }

  pub unsafe fn cmp_full(l: *const u8, r: *const u8, rid_off: usize) -> Ordering {
    let l_rid = *(l.add(rid_off) as *const Rid);
    let r_rid = *(r.add(rid_off) as *const Rid);
    Self::cmp(l, r).then(l_rid.cmp(&r_rid))
  }
}

// `x` should be like data_rid consecutively stored(have the same layout as in IndexPage::data)
// return the first index `i` that the `i`th element is the first element > `x` (in the sense of `cmp_full`)
// if caller can guarantee `x` exists in `ip`, and the elements in `ip` are strictly ascending, then the `i - 1`th element the only element == `x`
pub unsafe fn upper_bound<const T: BareTy>(ip: &IndexPage, x: *const u8) -> usize {
  let (mut i, count) = (0, ip.count as usize);
  let (slot_size, rid_off) = (ip.slot_size() as usize, ip.rid_off as usize);
  while i < count {
    if Cmp::<{ T }>::cmp_full(x, ip.data.as_ptr().add(i * slot_size), rid_off) == Ordering::Less { break; }
    i += 1;
  }
  i
}