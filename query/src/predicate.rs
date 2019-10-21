use chrono::NaiveDate;

use common::{*, Error::*, BareTy::*, CmpOp::*};
use syntax::ast::*;
use physics::*;
use crate::is_null;

macro_rules! handle_op {
  ($cmp: ident, $op:expr, $p: ident, $l: expr, $r: expr) => {
    match $op {
      Lt => $cmp!(<, $p, $l, $r), Le => $cmp!(<=, $p, $l, $r), Ge => $cmp!(>=, $p, $l, $r),
      Gt => $cmp!(>, $p, $l, $r), Eq => $cmp!(==, $p, $l, $r), Ne => $cmp!(!=, $p, $l, $r),
    }
  };
}

// the pointer from IndexPage cannot be passed to predicate!
// It is just the data ptr, but all these predicate accept the pointer to the beginning of the whole data slot

// assume both lhs and rhs belongs to tp's table, so ColRef::table is not checked
pub unsafe fn one_predicate<'a>(e: &Cond<'a>, tp: &TablePage) -> Result<'a, Box<dyn Fn(*const u8) -> bool>> {
  let tp = tp.pr();
  let l = tp.get_ci(e.lhs_col().col)?;
  let l_id = l.idx(&tp.cols) as u8; // reduce the size of lambda closure, do conversion inside lambda
  let l_off = l.off;
  match *e {
    Cond::Cmp(op, _, r) => match r {
      Atom::Lit(r) => {
        macro_rules! cmp {
          ($op: tt, $p: ident, $l: expr, $r: expr) => { Ok(box move |$p| !is_null($p, l_id as u32) && $l $op $r) };
        }
        // the match logic is basically the same as the logic in `fill_ptr`, though the content is different
        match (l.ty.ty, r.lit()) {
          (_, Lit::Null) => Ok(box |_| false), // comparing with null always returns false
          (Bool, Lit::Bool(v)) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const bool), v),
          (Int, Lit::Number(v)) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const i32), v as i32),
          (Float, Lit::Number(v)) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const f32), v as f32),
          (Date, Lit::Str(v)) => {
            let date = db::date(v)?;
            handle_op!(cmp, op, p, *(p.add(l_off as _) as *const NaiveDate), date)
          }
          (VarChar, Lit::Str(v)) => {
            let v = Box::<str>::from(v);
            handle_op!(cmp, op, p, str_from_db(p.add(l_off as _)), v.as_ref())
          }
          (expect, r) => return Err(RecordLitTyMismatch { expect, actual: r.ty() })
        }
      }
      Atom::ColRef(r) => {
        let r = tp.get_ci(r.col)?;
        let r_id = r.idx(&tp.cols) as u16;
        let r_off = r.off;
        macro_rules! cmp {
          ($op: tt, $p: ident, $l: expr, $r: expr) => { Ok(box move |$p| !is_null($p, l_id as u32) && !is_null($p, r_id as u32) && $l $op $r) };
        }
        match (l.ty.ty, r.ty.ty) {
          (Bool, Bool) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const bool), *(p.add(r_off as _) as *const bool)),
          (Int, Int) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const i32), *(p.add(r_off as _) as *const i32)),
          (Float, Float) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const f32), *(p.add(r_off as _) as *const f32)),
          (Int, Float) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const i32) as f32, *(p.add(r_off as _) as *const f32)),
          (Float, Int) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const f32), *(p.add(r_off as _) as *const i32) as f32),
          (Date, Date) => handle_op!(cmp, op, p, *(p.add(l_off as _) as *const NaiveDate), *(p.add(r_off as _) as *const NaiveDate)),
          (VarChar, VarChar) => handle_op!(cmp, op, p, str_from_db(p.add(l_off as _)), str_from_db(p.add(r_off as _))),
          (expect, actual) => return Err(RecordTyMismatch { expect, actual })
        }
      }
    },
    Cond::Null(_, null) => Ok(if null { box move |p| is_null(p, l_id as u32) } else { box move |p| !is_null(p, l_id as u32) }),
    Cond::Like(_, like) => {
      if l.ty.ty != VarChar { return Err(InvalidLikeTy(l.ty.ty)); }
      let re = db::like2re(like)?;
      Ok(box move |p| !is_null(p, l_id as u32) && re.is_match(str_from_db(p.add(l_off as _))))
    }
  }
}

pub unsafe fn cross_predicate<'a>(op: CmpOp, col: (&ColInfo, &ColInfo), tp: (&TablePage, &TablePage)) -> Result<'a, Box<dyn Fn((*const u8, *const u8)) -> bool>> {
  let (l, r) = col;
  let (l_id, r_id) = (l.idx(&tp.0.cols) as u16, r.idx(&tp.1.cols) as u16);
  let (l_off, r_off) = (l.off, r.off);
  macro_rules! cmp {
    ($op: tt, $p: ident, $l: expr, $r: expr) => { Ok(box move |$p| !is_null($p.0, l_id as u32) && !is_null($p.1, r_id as u32) && $l $op $r) };
  }
  match (l.ty.ty, r.ty.ty) {
    (Bool, Bool) => handle_op!(cmp, op, p, *(p.0.add(l_off as _) as *const bool), *(p.1.add(r_off as _) as *const bool)),
    (Int, Int) => handle_op!(cmp, op, p, *(p.0.add(l_off as _) as *const i32), *(p.1.add(r_off as _) as *const i32)),
    (Float, Float) => handle_op!(cmp, op, p, *(p.0.add(l_off as _) as *const f32), *(p.1.add(r_off as _) as *const f32)),
    (Int, Float) => handle_op!(cmp, op, p, *(p.0.add(l_off as _) as *const i32) as f32, *(p.1.add(r_off as _) as *const f32)),
    (Float, Int) => handle_op!(cmp, op, p, *(p.0.add(l_off as _) as *const f32), *(p.1.add(r_off as _) as *const i32) as f32),
    (Date, Date) => handle_op!(cmp, op, p, *(p.0.add(l_off as _) as *const NaiveDate), *(p.1.add(r_off as _) as *const NaiveDate)),
    (VarChar, VarChar) => handle_op!(cmp, op, p, str_from_db(p.0.add(l_off as _)), str_from_db(p.1.add(r_off as _))),
    (expect, actual) => return Err(RecordTyMismatch { expect, actual })
  }
}

pub unsafe fn one_where<'a>(where_: &[Cond<'a>], tp: &TablePage) -> Result<'a, impl Fn(*const u8) -> bool> {
  let mut preds = Vec::with_capacity(where_.len());
  for cond in where_ {
    let (l, r) = (cond.lhs_col(), cond.rhs_col_op().map(|x| x.0));
    if let Some(t) = l.table { if t != tp.name() { return Err(NoSuchTable(t)); } }
    if let Some(&ColRef { table: Some(t), .. }) = r { if t != tp.name() { return Err(NoSuchTable(t)); } }
    // table name is checked before, col name & type & value format/size all checked in one_predicate
    preds.push(one_predicate(cond, tp)?);
  }
  Ok(and(preds))
}

pub fn and<T: Copy>(ps: Vec<Box<dyn Fn(T) -> bool>>) -> impl Fn(T) -> bool {
  move |t| ps.iter().all(|p| p(t))
}