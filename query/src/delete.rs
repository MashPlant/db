use chrono::NaiveDate;
use unchecked_unwrap::UncheckedUnwrap;

use common::{*, BareTy::*, Error::*};
use syntax::ast::{*, CmpOp::*};
use physics::*;
use db::Db;
use index::Index;
use crate::{fill_ptr, handle_all, predicate::{one_predicate, and}};
use std::ptr::NonNull;

pub fn delete(d: &Delete, db: &mut Db) -> Result<()> {
  unsafe {
    let ti = db.get_ti(d.table)?;
    let tp = db.get_page::<TablePage>(ti.meta as usize);
    let mut preds = Vec::with_capacity(d.where_.len());
    for e in &d.where_ {
      let (l, r) = (e.lhs_col(), e.rhs_col());
      if let Some(table) = l.table {
        if table != d.table { return Err(NoSuchTable(table.into())); }
      }
      if let Some(&ColRef { table: Some(table), .. }) = r {
        if table != d.table { return Err(NoSuchTable(table.into())); }
      }
      preds.push(one_predicate(e, tp)?); // col name & type & value format/size all checked here
    }
    let _pred = and(preds);
    // todo don't forget to deallocate
    Ok(())
  }
}

// this `NonNull<u8>` is the `data_rid` from the IndexPage
unsafe fn collect(where_: &Vec<Expr>, tp: &mut TablePage, db: &mut Db) -> Vec<NonNull<u8>> {
  if let Some(rs) = {
    let mut iter = where_.iter();
    let tp_id = db.id_of(tp) as u32;
    loop {
      if let Some(e) = iter.next() { // expanded `for` loop, to make use of `loop`'s `break` value
        match e {
          Expr::Cmp(op, l, Atom::Lit(r)) => match r {
            Lit::Null => {}
            _ => {
              // safe because `one_predicate` have verified the name
              let col = tp.get_ci(l.col).unchecked_unwrap();
              let col_id = tp.id_of(col) as u32; // it may seem a little cumbersome to calculate id again, but doesn't matter much
              if col.index != !0 {
                let buf = Align4U8::new(col.ty.size() as usize);
                // safe because `one_predicate` have verified the type & value format/size
                fill_ptr(buf.ptr, col.ty, *r).unchecked_unwrap();
                let mut rs = Vec::new();
                macro_rules! handle {
                  ($ty: ident) => {{
                    let mut index = Index::<{ $ty }>::new(db, Rid::new(tp_id, col_id));
                    match op {
                      Lt => {
                        let (mut it, end) = (index.iter(), index.lower_bound(buf.ptr));
                        while let Some(data) = it.next() {
                          rs.push(data);
                          if it == end { break; }
                        }
                        rs
                      },
                      Le => {
                        let (mut it, end) = (index.iter(), index.upper_bound(buf.ptr));
                        while let Some(data) = it.next() {
                          rs.push(data);
                          if it == end { break; }
                        }
                        rs
                      },
                      Ge => {
                        let mut it = index.lower_bound(buf.ptr);
                        while let Some(data) = it.next() { rs.push(data); }
                        rs
                      },
                      Gt => {
                        let mut it = index.upper_bound(buf.ptr);
                        while let Some(data) = it.next() { rs.push(data); }
                        rs
                      },
                      Eq => {
                        let (mut it, end) = (index.lower_bound(buf.ptr), index.upper_bound(buf.ptr));
                        while let Some(data) = it.next() {
                          rs.push(data);
                          if it == end { break; }
                        }
                        rs
                      },
                      Ne => continue, // can't optimize with index, the `break Some(...)` will not be executed
                    }
                  }};
                }
                break Some(handle_all!(col.ty.ty, handle));
              }
            }
          }
          _ => {}
        }
      } else { break None; }
    }
  } {} else {}
  unimplemented!()
//  if let Some(res) = try_collect_with_index() {
//    unimplemented!()
//  } else {
//    unimplemented!()
//  }
}