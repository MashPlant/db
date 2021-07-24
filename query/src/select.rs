use unchecked_unwrap::UncheckedUnwrap;
use std::{fmt::Write, mem};

use common::{*, BareTy::*, Error::*, AggOp::*, CmpOp::*};
use syntax::ast::*;
use physics::*;
use db::{Db, is_null};
use crate::{predicate::{and, one_predicate, cross_predicate}, filter::filter};
use chrono::NaiveDate;
use ordslice::Ext;

#[derive(Copy, Clone)]
pub struct Col<'a> {
  // if op == Some(CountAll), `ci` is None, otherwise `ci` will always be Some
  pub op: Option<AggOp>,
  pub ci: Option<(u32, &'a ColInfo)>,
}

pub struct SelectResult<'a> {
  pub cols: Vec<Col<'a>>,
  // `data` is a 2-d array, dim = cols.len() * (data.len() / cols.len()) (data.len() / cols.len() is row_count())
  pub data: Vec<CLit<'a>>,
}

impl SelectResult<'_> {
  // `data` is 2-d array of dimension = tbls.len() * (data.len() / tbls.len())
  // tbls[i] <-> data[i], both belongs to a table
  unsafe fn new<'a>(db: &Db, tbls: &[Vec<Col<'a>>], data: &[*const u8]) -> SelectResult<'a> {
    let result_num = data.len() / tbls.len();
    // if has agg, all col should have agg (checked in mk_tbls)
    let has_agg = tbls.iter().flatten().any(|col| col.op.is_some());
    let data = if has_agg {
      tbls.iter().enumerate().flat_map(|(idx, tbl)| {
        tbl.iter().map(move |col| {
          // avg, sum, min, max, count should ignore null, if none is not null, all except count should return null, count should return 0
          // avg's denominator should also ignore null
          // count(*) should not ignore null
          let op = col.op.unchecked_unwrap();
          match op {
            Avg | Sum => { // only accept Int, Float, checked in mk_tbls
              let (ci_id, ci) = col.ci.unchecked_unwrap();
              let mut sum = 0.0; // use f64 for better precision (cover i32)
              let mut notnull_cnt = 0;
              for i in 0..result_num {
                let data = *data.get_unchecked(i * tbls.len() + idx);
                if !is_null(data, ci_id) {
                  let ptr = data.add(ci.off as usize);
                  match ci.ty { int!() => sum += *(ptr as *const i32) as f64, float!() => sum += *(ptr as *const f32) as f64, _ => impossible!() }
                  notnull_cnt += 1;
                }
              }
              CLit::new(if notnull_cnt == 0 { Lit::Null } else { Lit::Number(if op == Avg { sum / notnull_cnt as f64 } else { sum }) })
            }
            Min | Max => {
              let (ci_id, ci) = col.ci.unchecked_unwrap();
              let it = (0..result_num).filter_map(|i| {
                let lit = db.data2lit(*data.get_unchecked(i * tbls.len() + idx), ci_id, ci);
                if lit.is_null() { None } else { Some(lit) }
              });
              // can't use function reference directly because `cmp` is unsafe
              if op == Max { it.max_by(|l, r| l.cmp(*r)) } else { it.min_by(|l, r| l.cmp(*r)) }
                .unwrap_or(CLit::new(Lit::Null))
            }
            Count => CLit::new(Lit::Number((0..result_num).filter(|&i| {
              !is_null(*data.get_unchecked(i * tbls.len() + idx), col.ci.unchecked_unwrap().0)
            }).count() as f64)),
            CountAll => CLit::new(Lit::Number(result_num as f64)),
          }
        })
      }).collect()
    } else {
      let row = (tbls.iter().map(|tbl| tbl.len())).sum::<usize>();
      let mut ret = Vec::<CLit>::with_capacity(result_num * row);
      ret.set_len(result_num * row);
      for i in 0..result_num {
        let mut j = 0;
        for (idx, tbl) in tbls.iter().enumerate() {
          let data = *data.get_unchecked(i * tbls.len() + idx);
          for col in tbl {
            let (ci_id, ci) = col.ci.unchecked_unwrap();
            ret.as_mut_ptr().add(i * row + j).write(db.data2lit(data, ci_id, ci));
            j += 1;
          }
        }
      }
      ret
    };
    SelectResult { cols: tbls.iter().flatten().copied().collect(), data }
  }

  pub fn row_count(&self) -> usize {
    self.data.len().checked_div(self.cols.len()).unwrap_or(0)
  }

  pub fn csv(&self) -> String {
    unsafe {
      let mut csv = String::new();
      for &Col { op, ci, .. } in &self.cols {
        if let Some((_, ci)) = ci {
          if let Some(op) = op { write!(csv, "{}({})", op.name(), ci.name()).unchecked_unwrap(); } else { csv += ci.name(); }
        } else { csv += "count(*)"; }
        csv.push(',');
      }
      (csv.pop(), csv.push('\n'));
      for i in 0..self.row_count() {
        let row = self.data.get_unchecked(i * self.cols.len()..(i + 1) * self.cols.len());
        for lit in row {
          match lit.lit() { // some tiny modifications to Lit's `debug` method
            Lit::Null => {}
            Lit::Str(s) => {
              csv.reserve(s.len() + 2);
              csv.push('"');
              for ch in s.chars() {
                if ch == '"' { csv.push('"'); } // csv format, "" to escape "
                csv.push(ch);
              }
              csv.push('"');
            }
            _ => write!(csv, "{:?}", lit).unchecked_unwrap(),
          }
          csv.push(',');
        }
        (csv.pop(), csv.push('\n'));
      }
      (csv.pop(), csv).1
    }
  }
}

struct SelectCtx<'a, 'b> {
  tbls: IndexMap<&'a str, (u32, &'b TablePage)>,
  cols: HashMap<&'a str, Option<(&'b TablePage, &'b ColInfo, usize)>>,
}

impl<'a, 'b> SelectCtx<'a, 'b> {
  unsafe fn one_where(&self, cr: &ColRef<'a>) -> Result<'a, (&'b TablePage, &'b ColInfo, usize)> {
    if let Some(t) = cr.table {
      if let Some((tbl_idx_l, _, &tp)) = self.tbls.get_full(t) {
        Ok((tp.1.pr(), tp.1.pr().get_ci(cr.col)?, tbl_idx_l))
      } else { Err(NoSuchTable(t)) }
    } else {
      match self.cols.get(cr.col) {
        Some(&Some((tp, ci, tbl_idx_l))) => Ok((tp.pr(), ci.pr(), tbl_idx_l)),
        Some(None) => Err(AmbiguousCol(cr.col)),
        None => Err(NoSuchCol(cr.col)),
      }
    }
  }

  // the validity of AggOp is checked here
  unsafe fn mk_tbls(&self, ops: &Option<Vec<Agg<'a>>>) -> Result<'a, Vec<Vec<Col<'b>>>> {
    if let Some(ops) = ops {
      if ops.iter().any(|agg| agg.op.is_some()) != ops.iter().all(|agg| agg.op.is_some()) {
        return Err(MixedSelect);
      }
      let mut ret = vec![vec![]; self.tbls.len()];
      for &Agg { op, col } in ops {
        if op == Some(CountAll) {
          // I admit it is quite ugly...
          ret.get_unchecked_mut(0).push(Col { op, ci: None });
        } else {
          let (tp, ci, idx) = self.one_where(&col)?;
          if let Some(op) = op {
            if op == Avg || op == Sum {
              match ci.ty { int!() | float!() => {} col => return Err(InvalidAgg { col, op }), }
            }
          }
          ret.get_unchecked_mut(idx).push(Col { op, ci: Some((ci.idx(&tp.cols), ci)) });
        }
      }
      Ok(ret)
    } else { // select *
      Ok(self.tbls.iter().map(|(_, &(_, tp))| {
        tp.cols().iter().enumerate().map(|(ci_id, ci)| Col { op: None, ci: Some((ci_id as u32, ci)) }).collect()
      }).collect())
    }
  }
}

pub fn select<'a, 'b>(s: &Select<'a>, db: &'b Db) -> Result<'a, SelectResult<'b>> {
  unsafe {
    let db = db.pr();
    let tbl_num = s.tables.len();
    if tbl_num == 0 { return Ok(SelectResult { cols: vec![], data: vec![] }); }
    macro_rules! at { ($arr: expr, $x: expr, $y: expr) => { $arr.get_unchecked_mut($x * tbl_num + $y) }; }
    let mut tbls = IndexMap::default();
    let mut cols = HashMap::default();
    for (idx, &t) in s.tables.iter().enumerate() {
      let (tp_id, tp) = db.get_tp(t)?;
      if tbls.insert(t, (tp_id, &*tp.p())).is_some() { return Err(DupTable(t)); }
      for ci in tp.cols() {
        // if it exist, make it None; if it doesn't exist, insert it
        cols.entry(ci.name()).and_modify(|x| *x = None).or_insert(Some((&*tp.p(), ci, idx)));
      }
    }
    let ctx = SelectCtx { tbls, cols };

    let mut one_preds = Vec::with_capacity(tbl_num);
    // `cross_preds` is 2-d array, dim = tbl_num * tbl_num
    // cross_preds[x][y] means a predicate that accept (x, y), only use lower parts (x > y)
    let mut cross_preds = Vec::with_capacity(tbl_num * tbl_num);
    // `cross_cols` store the col info of `cross_preds`, for optimization use
    // if one of the CmpOp is not Ne and their types are the same (ignore size) and are both fixed, it will be put into `cross_cols`, and we can use binary search to locate RHS
    let mut cross_cols = vec![None; tbl_num * tbl_num];
    for _ in 0..tbl_num { one_preds.push(vec![]); } // Box<Fn> is not Clone, so must use loop to push
    for _ in 0..tbl_num * tbl_num { cross_preds.push(vec![]); }
    let mut one_wheres = vec![vec![]; tbl_num];
    for cond in &s.where_ {
      let (l, r) = (cond.lhs_col(), cond.rhs_col_op());
      let (mut tp_l, mut ci_l, mut idx_l) = ctx.one_where(l)?;
      if let Some(((mut tp_r, mut ci_r, mut idx_r), mut op)) = {
        if let Some((r, op)) = r {
          Some((ctx.one_where(r)?, op)).filter(|((_, _, idx_r), _)| *idx_r != idx_l)
        } else { None }
      } { // not in one table
        if idx_l < idx_r {
          op = op.rev();
          mem::swap(&mut tp_l, &mut tp_r);
          mem::swap(&mut ci_l, &mut ci_r);
          mem::swap(&mut idx_l, &mut idx_r);
        }
        at!(cross_preds, idx_l, idx_r).push(cross_predicate(db.pr(), op, (ci_l, ci_r), (tp_l, tp_r))?);
        if op != Ne && !ci_l.ty.is_varchar() && !ci_r.ty.is_varchar() && ci_l.ty.fix_ty().ty == ci_r.ty.fix_ty().ty {
          at!(cross_cols, idx_l, idx_r).get_or_insert((op, ci_l, ci_r)); // store the first expr
        }
      } else { // in one table
        one_preds.get_unchecked_mut(idx_l).push(one_predicate(db.pr(), cond, tp_l)?);
        one_wheres.get_unchecked_mut(idx_l).push(cond);
      }
    }

    let mut cross_preds = cross_preds.into_iter().map(|p| and(p)).collect::<Vec<_>>();
    let mut one_results = vec![vec![]; tbl_num];
    for (idx, pred) in one_preds.into_iter().enumerate() { // idx in 0..tbl_num
      let (_, &(tp_id, tp)) = ctx.tbls.get_index(idx).unchecked_unwrap();
      let where_ = one_wheres.get_unchecked(idx);
      let one_result = one_results.get_unchecked_mut(idx);
      filter(db, where_, tp_id, and(pred), |x, _| {
        // remove some null data, it can optimize a little, but mainly for making later handling easier
        // if it participate in any comparison, then reject null results, so later the sort + binary search can avoid handling null
        if (0..idx).all(|idx1| at!(cross_cols, idx, idx1).map(|(_, ci, _)| !is_null(x, ci.idx(&tp.cols))).unwrap_or(true)) &&
          (idx + 1..tbl_num).all(|idx1| at!(cross_cols, idx1, idx).map(|(_, _, ci)| !is_null(x, ci.idx(&tp.cols))).unwrap_or(true)) {
          one_result.push(x as *const u8);
        }
        Ok(())
      }, true).unchecked_unwrap();
    }

    let res0 = one_results.get_unchecked(0);
    let mut final_ = Vec::<*const u8>::with_capacity(res0.len() * tbl_num);
    final_.set_len(res0.len() * tbl_num);
    for (i, &x) in res0.iter().enumerate() {
      final_.as_mut_ptr().add(i * tbl_num).write(x);
    }

    for idx_r in 1..one_results.len() {
      let rs = one_results.get_unchecked_mut(idx_r);
      let mut new_final_ = Vec::<*const u8>::new();
      macro_rules! join {
        ($old_row: expr, $range: expr) => {
          for &r in rs.get_unchecked($range) {
            if (0..idx_r).all(|idx_l| at!(cross_preds, idx_r, idx_l)((r, *$old_row.add(idx_l)))) {
              let old_len = new_final_.len();
              new_final_.reserve(tbl_num);
              new_final_.set_len(old_len + tbl_num);
              new_final_.as_mut_ptr().add(old_len).copy_from_nonoverlapping($old_row, tbl_num);
              *new_final_.get_unchecked_mut(old_len + idx_r) = r;
            }
          }
        };
      }
      if let Some((idx_l, (op, ci_r, ci_l))) = (0..idx_r).filter_map(|idx_l| at!(cross_cols,idx_r, idx_l).map(|x| (idx_l, x))).next() {
        let (off_l, off_r) = (ci_l.off as usize, ci_r.off as usize);
        match ci_r.ty.fix_ty().ty {
          Bool => rs.sort_unstable_by_key(|&x| *(x.add(off_r) as *const bool)),
          Int => rs.sort_unstable_by_key(|&x| *(x.add(off_r) as *const i32)),
          // note that both `l` and `r` use `off_r` here, because they are both from the `rs`
          Float => rs.sort_unstable_by(|&l, &r| fcmp(*(l.add(off_r) as *const f32), *(r.add(off_r) as *const f32))),
          Date => rs.sort_unstable_by_key(|&x| *(x.add(off_r) as *const NaiveDate)),
          Char => rs.sort_unstable_by_key(|&x| str_from_db(x.add(off_r))),
        }
        for old_idx in 0..(final_.len() / tbl_num) {
          let old_row = final_.as_ptr().add(old_idx * tbl_num);
          let l = (*old_row.add(idx_l)).add(off_l);
          let rg = match ci_r.ty.fix_ty().ty {
            Bool => rs.equal_range_by(|&r| (*(r.add(off_r) as *const bool)).cmp(&*(l as *const bool))),
            Int => rs.equal_range_by(|&r| (*(r.add(off_r) as *const i32)).cmp(&*(l as *const i32))),
            Float => rs.equal_range_by(|&r| fcmp(*(r.add(off_r) as *const f32), *(l as *const f32))),
            Date => rs.equal_range_by(|&r| (*(r.add(off_r) as *const NaiveDate)).cmp(&*(l as *const NaiveDate))),
            Char => rs.equal_range_by(|&r| str_from_db(r.add(off_r)).cmp(str_from_db(l))),
          };
          let rg = match op {
            Lt => 0..rg.start, Le => 0..rg.end, Ge => rg.start..rs.len(), Gt => rg.end..rs.len(), Eq => rg, Ne => impossible!(),
          };
          join!(old_row, rg);
        }
      } else {
        for old_idx in 0..(final_.len() / tbl_num) {
          let old_row = final_.as_ptr().add(old_idx * tbl_num);
          join!(old_row, ..);
        }
      }
      final_ = new_final_;
    }
    Ok(SelectResult::new(db, &ctx.mk_tbls(&s.ops)?, &final_))
  }
}