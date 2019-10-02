use unchecked_unwrap::UncheckedUnwrap;
use std::borrow::Borrow;

use common::{*, BareTy::*};
use syntax::ast::{*, CmpOp::*};
use physics::*;
use db::{Db, fill_ptr};
use index::{Index, handle_all};

// return true for successfully filtered with index
unsafe fn try_filter_with_index<'a, E: Borrow<Expr<'a>>>(where_: &[E], tp: (u32, &TablePage), db: &mut Db,
                                                         pred: &impl Fn(*const u8) -> bool, f: &mut impl FnMut(*mut u8, Rid)) -> bool {
  let (tp_id, tp) = tp;
  for e in where_ {
    let e = e.borrow();
    if let Expr::Cmp(op, l, Atom::Lit(r)) = e {
      match r {
        Lit::Null => {}
        _ => {
          // safe because `one_predicate` have verified the name
          let ci = tp.pr().get_ci(l.col).unchecked_unwrap();
          let ci_id = ci.idx(&tp.cols);
          if ci.index != !0 {
            let buf = Align4U8::new(ci.ty.size() as usize);
            // safe because `one_predicate` have verified the type & value format/size
            fill_ptr(buf.ptr, ci.ty, *r).unchecked_unwrap();
            macro_rules! handle {
              ($ty: ident) => {{
                let mut index = Index::<{ $ty }>::new(db, Rid::new(tp_id, ci_id));
                match op {
                  Lt => {
                    let (mut it, end) = (index.iter(), index.lower_bound(buf.ptr));
                    while let Some((_, rid)) = it.next() {
                      let ptr = db.get_data_slot(tp, rid);
                      if pred(ptr) { f(ptr, rid); }
                      if it == end { break; }
                    }
                  },
                  Le => {
                    let (mut it, end) = (index.iter(), index.upper_bound(buf.ptr));
                    while let Some((_, rid)) = it.next() {
                      let ptr = db.get_data_slot(tp, rid);
                      if pred(ptr) { f(ptr, rid); }
                      if it == end { break; }
                    }
                  },
                  Ge => {
                    let mut it = index.lower_bound(buf.ptr);
                    while let Some((_, rid)) = it.next() {
                      let ptr = db.get_data_slot(tp, rid);
                      if pred(ptr) { f(ptr, rid); }
                    }
                  },
                  Gt => {
                    let mut it = index.upper_bound(buf.ptr);
                    while let Some((_, rid)) = it.next(){
                      let ptr = db.get_data_slot(tp, rid);
                      if pred(ptr) { f(ptr, rid); }
                    }
                  },
                  Eq => {
                    let (mut it, end) = (index.lower_bound(buf.ptr), index.upper_bound(buf.ptr));
                    while let Some((_, rid)) = it.next() {
                      let ptr = db.get_data_slot(tp, rid);
                      if pred(ptr) { f(ptr, rid); }
                      if it == end { break; }
                    }
                  },
                  Ne => continue, // can't optimize with index
                }
              }};
            }
            handle_all!(ci.ty.ty, handle);
            return true;
          }
        }
      }
    }
  }
  false
}

// guarantee the `*mut u8` passed to f only comes from DataPage, not from IndexPage
pub(crate) unsafe fn filter<'a, E: Borrow<Expr<'a>>>(where_: &[E], tp: (u32, &TablePage), db: &mut Db,
                                                     pred: impl Fn(*const u8) -> bool, mut f: impl FnMut(*mut u8, Rid)) {
  if !try_filter_with_index(where_, tp, db, &pred, &mut f) {
    for (data, rid) in db.record_iter(tp.1) {
      if pred(data.as_ptr()) { f(data.as_ptr(), rid); }
    }
  }
}