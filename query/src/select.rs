use std::fmt;
use chrono::NaiveDate;

use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use physics::*;
use db::Db;
use crate::predicate::{and, one_predicate, cross_predicate};
use crate::filter::filter;
use unchecked_unwrap::UncheckedUnwrap;

pub struct SelectResult {
  // tbl[i] correspond to a table
  pub tbl: Vec<Vec<&'static ColInfo>>,
  // data[i] is one line, data[i].len() == tbl.len()
  pub data: Vec<Vec<*const u8>>,
}

impl fmt::Display for SelectResult {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    unsafe {
      for tbl in &self.tbl {
        for &col in tbl {
          write!(f, "{}, ", col.name())?;
        }
      }
      writeln!(f)?;
      for data in &self.data {
        debug_assert_eq!(data.len(), self.tbl.len());
        for (&data, tbl) in data.iter().zip(self.tbl.iter()) {
          for &col in tbl {
            let ptr = data.add(col.off as usize);
            match col.ty.ty {
              Int => write!(f, "{}, ", *(ptr as *const i32)),
              Bool => write!(f, "{}, ", *(ptr as *const bool)),
              Float => write!(f, "{}, ", *(ptr as *const f32)),
              Char | VarChar => write!(f, "'{}', ", str_from_parts(ptr.add(1), *ptr as usize)),
              Date => write!(f, "{}, ", *(ptr as *const NaiveDate)),
            }?;
          }
        }
        writeln!(f)?;
      }
      Ok(())
    }
  }
}

struct InsertCtx<'a> {
  tbls: IndexMap<&'a str, &'a TablePage>,
  cols: HashMap<&'a str, Option<(&'a TablePage, &'a ColInfo, usize)>>,
}

unsafe fn one_where<'a>(cr: &ColRef, ctx: &InsertCtx) -> Result<(&'a TablePage, &'a ColInfo, usize)> {
  if let Some(t) = cr.table {
    if let Some((tbl_idx, _, &tp)) = ctx.tbls.get_full(t) {
      Ok((tp.pr(), tp.pr().get_ci(cr.col)?, tbl_idx))
    } else { Err(NoSuchTable(t.into())) }
  } else {
    match ctx.cols.get(cr.col) {
      Some(&Some((tp, ci, tbl_idx))) => Ok((tp.pr(), ci.pr(), tbl_idx)),
      Some(None) => Err(AmbiguousCol(cr.col.into())),
      None => Err(NoSuchCol(cr.col.into())),
    }
  }
}

unsafe fn mk_tbl<'a>(ops: &Option<Vec<Agg>>, ctx: &InsertCtx) -> Result<Vec<Vec<&'a ColInfo>>> {
  if let Some(ops) = &ops {
    let mut ret = vec![vec![]; ctx.tbls.len()];
    for op in ops {
      match op.op {
        AggOp::None => {
          let (_, ci, idx) = one_where(&op.col, ctx)?;
          debug_assert!(idx < ret.len());
          ret.get_unchecked_mut(idx).push(ci);
        }
        _ => unimplemented!()
      }
    }
//    col
    Ok(ret)
  } else { // select *
    Ok(ctx.tbls.iter()
      .map(|(_, &tp)| (0..tp.col_num as usize).map(|i| tp.cols.get_unchecked(i).prc()).collect())
      .collect())
  }
}

pub fn select(s: &Select, db: &mut Db) -> Result<SelectResult> {
  unsafe {
    debug_assert!(s.tables.len() >= 1);
    let mut tbls = IndexMap::default();
    let mut cols = HashMap::new();
    for (idx, &t) in s.tables.iter().enumerate() {
      let ti = db.get_ti(t)?;
      let tp = db.get_page::<TablePage>(ti.meta as usize);
      match tbls.entry(t) {
        IndexEntry::Occupied(_) => return Err(DupTable(t.into())),
        IndexEntry::Vacant(v) => { v.insert(tp.prc()); }
      }
      for i in 0..tp.col_num as usize {
        let ci = tp.pr().cols.get_unchecked(i);
        // if it exist, make it None; if it doesn't exist, insert it
        cols.entry(ci.name()).and_modify(|x| *x = None)
          .or_insert(Some((tp.prc(), ci, idx)));
      }
    }
    debug_assert_eq!(tbls.len(), s.tables.len());
    let ctx = InsertCtx { tbls, cols };
    let result_tbl = mk_tbl(&s.ops, &ctx)?;

    let mut one_preds = Vec::with_capacity(s.tables.len());
    let mut one_wheres = vec![vec![]; s.tables.len()];
    let mut cross_preds = HashMap::new();
    for _ in 0..s.tables.len() { one_preds.push(vec![]); } // Box<Fn> is not Clone
    for e in &s.where_ {
      let (l, r) = (e.lhs_col(), e.rhs_col());
      let (tp_l, ci_l, tbl_idx_l) = one_where(l, &ctx)?;
      debug_assert!(tbl_idx_l < one_preds.len());
      if let Some((tp_r, ci_r, tbl_idx_r)) = {
        if let Some(r) = r {
          Some(one_where(r, &ctx)?).filter(|(_, _, tbl_idx_r)| *tbl_idx_r != tbl_idx_l)
        } else { None }
      } { // not in one table
        if let &Expr::Cmp(op, _, _) = e {
          cross_preds.entry((tbl_idx_l as u32, tbl_idx_r as u32)).or_insert_with(Vec::new)
            .push(cross_predicate(op, (ci_l, ci_r), (tp_l, tp_r))?);
        } else { debug_unreachable!() } // if expr have rhs col, it must have cmp op
      } else { // in one table
        one_preds.get_unchecked_mut(tbl_idx_l).push(one_predicate(e, tp_l)?);
        one_wheres.get_unchecked_mut(tbl_idx_l).push(e);
      }
    }
    let cross_preds = cross_preds.into_iter().map(|(k, v)| (k, and(v)))
      .collect::<HashMap<_, _>>();

    let mut one_results = ctx.tbls.values().zip(one_preds.into_iter())
      .zip(one_wheres.iter()).enumerate()
      .map(|(idx, ((&tp, pred), where_))| {
        let mut data = Vec::new();
        filter(where_, tp, db, and(pred), |data1, _| data.push(data1 as *const u8));
        (idx, data)
      }).collect::<Vec<_>>();
    one_results.sort_unstable_by_key(|x| x.1.len());
    let mut one_results = one_results.into_iter();
    let (mut tbl_idx_l, res_l) = one_results.next().unchecked_unwrap(); // there are at least 1 table
    let mut res_l = res_l.into_iter().map(|x| vec![x]).collect::<Vec<_>>();
    for (tbl_idx_r, res_r) in one_results {
      let old_res_l = std::mem::replace(&mut res_l, Vec::new());
      res_l.reserve(old_res_l.len() * res_r.len());
      if let Some(pred) = cross_preds.get(&(tbl_idx_l as u32, tbl_idx_r as u32)) {
        for l in old_res_l {
          for &r in &res_r {
            let data_l = *l.last().unchecked_unwrap();
            if pred((data_l, r)) {
              let mut tmp = l.clone();
              tmp.push(r);
              res_l.push(tmp);
            }
          }
        }
      } else {
        for l in old_res_l {
          for &r in &res_r {
            let mut tmp = l.clone();
            tmp.push(r);
            res_l.push(tmp);
          }
        }
      }
      tbl_idx_l = tbl_idx_r;
    }
    Ok(SelectResult { tbl: result_tbl, data: res_l })
  }
}


//    let preds = Vec::with_capacity(s.tables.len());

//    if s.tables.len() == 1 {
//      let table = s.tables[0];
//      let ti = db.get_ti(table)?;
//      let tp = db.get_page::<TablePage>(ti.meta as usize);
//      let pred = one_where(&s.where_, table, tp)?;
//      let col = if let Some(ops) = &s.ops {
//        let mut col = Vec::with_capacity(ops.len());
//        for op in ops {
//          match op.op {
//            AggOp::None => {
//              if let Some(t) = op.col.table {
//                if t != table { return Err(NoSuchTable(t.into())); }
//              }
//              col.push(&*tp.get_ci(op.col.col)?);
//            }
//            _ => unimplemented!()
//          }
//        }
//        col
//      } else { // select *
//        (0..tp.col_num as usize).map(|i| &*tp.cols.get_unchecked(i).p()).collect()
//      };
//      let mut data = Vec::new();
//      filter(&s.where_, tp, db, pred, |data1, _| data.push(data1 as *const u8));
//      Ok(SelectResult { col, data: vec![data] })
//    } else {
//      unimplemented!()
//    }