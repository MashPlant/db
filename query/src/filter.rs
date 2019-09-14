use unchecked_unwrap::UncheckedUnwrap;

use common::{*, BareTy::*};
use syntax::ast::{*, CmpOp::*};
use physics::*;
use db::Db;
use index::Index;
use crate::{fill_ptr, handle_all};

// return true for successfully filtered with index
unsafe fn try_filter_with_index(where_: &Vec<Expr>, tp: &mut TablePage, db: &mut Db,
                                pred: &impl Fn(*const u8) -> bool, f: &mut impl FnMut(*mut u8, Rid)) -> bool {
  let tp_id = db.id_of(tp) as u32;
  for e in where_ {
    if let Expr::Cmp(op, l, Atom::Lit(r)) = e {
      match r {
        Lit::Null => {}
        _ => {
          // safe because `one_predicate` have verified the name
          let col = tp.get_ci(l.col).unchecked_unwrap();
          let col_id = tp.id_of(col) as u32; // it may seem a little cumbersome to calculate id again, but doesn't matter much
          if col.index != !0 {
            let buf = Align4U8::new(col.ty.size() as usize);
            // safe because `one_predicate` have verified the type & value format/size
            fill_ptr(buf.ptr, col.ty, *r).unchecked_unwrap();
            macro_rules! handle {
              ($ty: ident) => {{
                let mut index = Index::<{ $ty }>::new(db, Rid::new(tp_id, col_id));
                match op {
                  Lt => {
                    let (mut it, end) = (index.iter(), index.lower_bound(buf.ptr));
                    while let Some((data, rid)) = it.next() {
                      // these two pointers (data.as_ptr() and db.get_data_slot(tp, rid)) should have the same content
                      // they just have different owner, former is from IndexPage, latter is from DataPage
                      if pred(data.as_ptr()) { f(db.get_data_slot(tp, rid), rid); }
                      if it == end { break; }
                    }
                  },
                  Le => {
                    let (mut it, end) = (index.iter(), index.upper_bound(buf.ptr));
                    while let Some((data, rid)) = it.next() {
                      if pred(data.as_ptr()) { f(db.get_data_slot(tp, rid), rid); }
                      if it == end { break; }
                    }
                  },
                  Ge => {
                    let mut it = index.lower_bound(buf.ptr);
                    while let Some((data, rid)) = it.next() {
                      if pred(data.as_ptr()) { f(db.get_data_slot(tp, rid), rid); }
                    }
                  },
                  Gt => {
                    let mut it = index.upper_bound(buf.ptr);
                    while let Some((data, rid)) = it.next(){
                      if pred(data.as_ptr()) { f(db.get_data_slot(tp, rid), rid); }
                    }
                  },
                  Eq => {
                    let (mut it, end) = (index.lower_bound(buf.ptr), index.upper_bound(buf.ptr));
                    while let Some((data, rid)) = it.next() {
                      if pred(data.as_ptr()) { f(db.get_data_slot(tp, rid), rid); }
                      if it == end { break; }
                    }
                  },
                  Ne => continue, // can't optimize with index
                }
              }};
            }
            handle_all!(col.ty.ty, handle);
            return true;
          }
        }
      }
    }
  }
  false
}

// guarantee the `*mut u8` passed to f only comes from DataPage, not from IndexPage
pub(crate) unsafe fn filter(where_: &Vec<Expr>, tp: &mut TablePage, db: &mut Db,
                            pred: impl Fn(*const u8) -> bool, mut f: impl FnMut(*mut u8, Rid)) {
  if !try_filter_with_index(where_, tp, db, &pred, &mut f) {
    for (data, rid) in db.record_iter(tp) {
      if pred(data.as_ptr()) { f(data.as_ptr(), rid); }
    }
  }
}