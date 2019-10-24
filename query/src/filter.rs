use unchecked_unwrap::UncheckedUnwrap;
use std::borrow::Borrow;

use common::{*, BareTy::*, CmpOp::*};
use syntax::ast::*;
use physics::*;
use db::{Db, fill_ptr};
use index::{Index, handle_all};

// return true for successfully filtered with index
unsafe fn try_filter_with_index<'a>(db: &mut Db, where_: &[impl Borrow<Cond<'a>>], tp_id: u32,
                                    pred: &impl Fn(*const u8) -> bool, f: &mut impl FnMut(*mut u8, Rid) -> Result<'a, ()>) -> Result<'a, bool> {
  let tp = db.get_page::<TablePage>(tp_id);
  for cond in where_ {
    if let &Cond::Cmp(op, l, Atom::Lit(r)) = cond.borrow() {
      match r.lit() {
        Lit::Null => {}
        _ => {
          // safe because `one_predicate` have verified the name
          let ci = tp.pr().get_ci(l.col).unchecked_unwrap();
          let ci_id = ci.idx(&tp.cols);
          if ci.index != !0 {
            let buf = Align4U8::new(ci.ty.size() as usize);
            let is_only_pred = where_.len() == 1;
            // safe because `one_predicate` have verified the type & value format/size
            fill_ptr(buf.ptr, ci.ty, r).unchecked_unwrap();
            macro_rules! handle {
              ($ty: ident) => {{
                let mut index = Index::<{ $ty }>::new(db, tp_id, ci_id);
                match op {
                  Lt | Le | Eq => {
                    let (mut it, end) = match op {
                      Lt => (index.iter(), index.lower_bound(buf.ptr)),
                      Le => (index.iter(), index.upper_bound(buf.ptr)),
                      Eq => (index.lower_bound(buf.ptr), index.upper_bound(buf.ptr)),
                      _ => debug_unreachable!(),
                    };
                    while it != end {
                      let rid = it.next().unchecked_unwrap();
                      let ptr = db.get_data_slot(tp, rid);
                      if is_only_pred || pred(ptr) { f(ptr, rid)?; }
                    }
                  },
                  Ge | Gt => {
                    let mut it = if op == Ge { index.lower_bound(buf.ptr) } else { index.upper_bound(buf.ptr) };
                    while let Some(rid) = it.next() {
                      let ptr = db.get_data_slot(tp, rid);
                      if is_only_pred || pred(ptr) { f(ptr, rid)?; }
                    }
                  },
                  Ne => continue, // can't optimize with index
                }
              }};
            }
            handle_all!(ci.ty.ty, handle);
            return Ok(true);
          }
        }
      }
    }
  }
  Ok(false)
}

// guarantee the `*mut u8` passed to f only comes from DataPage, not from IndexPage
// if you want to modify index while iterating, you CANNOT modify while iterating, remember to set `use_index` = false
// if you want to delete the current data slot from data page while iterating, you CAN delete while iterating (due to the implementation)
pub(crate) unsafe fn filter<'a>(db: &mut Db, where_: &[impl Borrow<Cond<'a>>], tp_id: u32,
                                pred: impl Fn(*const u8) -> bool, mut f: impl FnMut(*mut u8, Rid) -> Result<'a, ()>,
                                use_index: bool) -> Result<'a, ()> {
  if !use_index || !try_filter_with_index(db, where_, tp_id, &pred, &mut f)? {
    let tp = db.get_page::<TablePage>(tp_id);
    for (data, rid) in db.record_iter(tp) { if pred(data) { f(data, rid)?; } }
  }
  Ok(())
}