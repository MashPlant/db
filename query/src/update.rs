use unchecked_unwrap::UncheckedUnwrap;
use regex::Regex;
use std::cmp::Ordering::*;

use common::{*, Error::*, BinOp::*, CmpOp::*, BareTy::*};
use syntax::ast::*;
use physics::*;
use db::{Db, is_null, hash_pks};
use index::{Index, handle_all};
use crate::{predicate::one_where, filter::filter, check_foreign_link, InsertCtx};

unsafe fn check<'a>(e: &Expr<'a>, tp: &mut TablePage, re_cache: &mut HashMap<&'a str, Regex>) -> Result<'a, LitTy> {
  match e {
    Expr::Atom(x) => Ok(match x {
      Atom::Lit(x) => x.lit().ty(),
      Atom::ColRef(col) => {
        if let Some(t) = col.table { if t != tp.name() { return Err(NoSuchTable(t)); } }
        let ci = tp.get_ci(col.col)?;
        match ci.ty {
          ColTy::FixTy(ty) => match ty.ty { Bool => LitTy::Bool, Int | Float => LitTy::Number, Date => LitTy::Date, Char => LitTy::Str },
          varchar!() => LitTy::Str,
        }
      }
    }),
    Expr::Null(x, _) => (check(x, tp, re_cache)?, Ok(LitTy::Bool)).1,
    Expr::Like(x, like) => {
      match check(x, tp, re_cache)? { LitTy::Str => {} ty => return Err(InvalidLikeTy1(ty)) };
      re_cache.insert(like, db::like2re(like)?);
      Ok(LitTy::Bool)
    }
    Expr::And(box (l, r)) | Expr::Or(box (l, r)) => {
      match check(l, tp, re_cache)? { LitTy::Bool => {} ty => return Err(IncompatibleLogic(ty)) };
      match check(r, tp, re_cache)? { LitTy::Bool => {} ty => return Err(IncompatibleLogic(ty)) };
      Ok(LitTy::Bool)
    }
    Expr::Cmp(op, box (l, r)) => {
      let (l, r) = (check(l, tp, re_cache)?, check(r, tp, re_cache)?);
      if l == r { Ok(LitTy::Bool) } else { Err(IncompatibleCmp { op: *op, l, r }) }
    }
    Expr::Bin(op, box (l, r)) => {
      match check(l, tp, re_cache)? { LitTy::Number => {} ty => return Err(IncompatibleBin { op: *op, ty }) };
      match check(r, tp, re_cache)? { LitTy::Number => {} ty => return Err(IncompatibleBin { op: *op, ty }) };
      Ok(LitTy::Number)
    }
  }
}

// if one of the operand is null, the result is null (including comparison, e.g., (null = null) evaluates to null, instead of false in select)
// the only exception is "is (not) null" check, it always return bool
// if arithmetic result is NaN, the result is null
unsafe fn eval<'a>(db: &Db, e: &Expr<'a>, tp: &mut TablePage, data: *const u8, re_cache: &HashMap<&'a str, Regex>) -> Lit<'a> {
  match e {
    Expr::Atom(x) => match x {
      Atom::Lit(x) => *x,
      Atom::ColRef(col) => {
        let ci = tp.get_ci(col.col).unchecked_unwrap();
        let ci_id = ci.idx(&tp.cols);
        db.data2lit(data, ci_id, ci)
      }
    }.lit(),
    Expr::Null(x, null) => Lit::Bool(eval(db, x, tp, data, re_cache).is_null() == *null),
    Expr::Like(x, like) => {
      let re = re_cache.get(like).unchecked_unwrap();
      let x = match eval(db, x, tp, data, re_cache) { Lit::Str(x) => x, _ => return Lit::Null };
      Lit::Bool(re.is_match(x))
    }
    Expr::And(box (l, r)) | Expr::Or(box (l, r)) => {
      let or = if let Expr::Or(_) = e { true } else { false };
      let l = match eval(db, l, tp, data, re_cache) { Lit::Bool(x) => x, _ => return Lit::Null };
      if or == l { return Lit::Bool(l); } // short circuit, true or _ / false and _
      // now it is false or _ / true and _, the result only depends on `r`
      let r = match eval(db, r, tp, data, re_cache) { Lit::Bool(x) => x, _ => return Lit::Null };
      Lit::Bool(r)
    }
    Expr::Cmp(op, box (l, r)) => {
      let l = eval(db, l, tp, data, re_cache);
      let r = eval(db, r, tp, data, re_cache);
      if l.is_null() || r.is_null() { return Lit::Null; };
      let cmp = l.cmp(&r); // `check` and null check above guarantees they have the same type
      Lit::Bool(match op { Lt => cmp == Less, Le => cmp != Greater, Ge => cmp != Less, Gt => cmp == Greater, Eq => cmp == Equal, Ne => cmp != Equal })
    }
    Expr::Bin(op, box (l, r)) => {
      // since we cannot have type mismatch here, if it is not Number, it can only be Null
      let l = match eval(db, l, tp, data, re_cache) { Lit::Number(x) => x, _ => return Lit::Null };
      let r = match eval(db, r, tp, data, re_cache) { Lit::Number(x) => x, _ => return Lit::Null };
      let res = match op { Add => l + r, Sub => l - r, Mul => l * r, Div => l / r, Mod => l % r, };
      if res.is_nan() { Lit::Null } else { Lit::Number(res) }
    }
  }
}

pub fn update<'a>(u: &Update<'a>, db: &mut Db) -> ModifyResult<'a, u32> {
  unsafe {
    let mut ctx = InsertCtx::new(db, u.table, None)?;
    let f_links = db.foreign_links_to(ctx.tp_id).collect::<Vec<_>>();
    let pred = one_where(db.pr(), &u.where_, ctx.tp)?;
    let mut re_cache = HashMap::new();
    let mut cols = Vec::with_capacity(u.sets.len());
    let mut vals = vec![CLit::new(Lit::Null); u.sets.len()]; // the initial value is useless (and not really necessary...)
    for (col, e) in &u.sets {
      cols.push(&*ctx.tp.get_ci(col)?);
      check(e, ctx.tp, &mut re_cache)?;
    }
    let slot_size = ctx.tp.size as usize;
    let buf = Align4U8::new(slot_size); // update to buf, then copy to db
    let mut cnt = 0;
    if let Err(e) = filter(db.pr(), &u.where_, ctx.tp_id, pred, |data, rid| {
      check_foreign_link(db, ctx.tp, data, &f_links)?;
      buf.ptr.copy_from_nonoverlapping(data, slot_size);
      for (idx, (_, e)) in u.sets.iter().enumerate() {
        let ci = *cols.get_unchecked(idx);
        let ci_id = ci.idx(&ctx.tp.cols);
        let val = CLit::new(eval(db, e, ctx.tp, data, &re_cache));
        *vals.get_unchecked_mut(idx) = val;
        if val.is_null() {
          if ci.flags.intersects(ColFlags::NOTNULL1) { return Err(PutNullOnNotNull); }
          bsset(buf.ptr as *mut u32, ci_id as usize);
        } else {
          bsdel(buf.ptr as *mut u32, ci_id as usize);
          if !ci.ty.is_varchar() { db.lit2ptr(buf.ptr.add(ci.off as usize), ci.ty.fix_ty(), val)?; }
        }
        ctx.check_col(buf.ptr, ci_id, val, Some(rid))?; // it won't conflict with the old value (`data`)
      }
      if ctx.pks.len() > 1 {
        ctx.pk_set.remove(&hash_pks(data, &ctx.pks));
        if !ctx.pk_set.insert(hash_pks(buf.ptr, &ctx.pks)) { return Err(PutDupOnPrimary); }
      }
      for (idx, &val) in vals.iter().enumerate() {
        if !val.is_null() { Db::varchar_ck(cols.get_unchecked(idx).ty, val)?; }
      }
      // now no error can occur
      for (idx, &val) in vals.iter().enumerate() {
        let ci = *cols.get_unchecked(idx);
        let ci_id = ci.idx(&ctx.tp.cols);
        if ci.ty.is_varchar() {
          let ptr = buf.ptr.add(ci.off as usize);
          let initialized = !is_null(data, ci_id); // this is the old value, null-bitset in new value (buf.ptr) is already set
          match val.lit() {
            Lit::Null => if initialized { db.free_varchar(ptr); }
            Lit::Str(s) => db.lit2varchar(ptr, s, true),
            _ => impossible!()
          }
        }
      }
      for (col, _) in &u.sets {
        let ci = ctx.tp.get_ci(col).unchecked_unwrap();
        let ci_id = ci.idx(&ctx.tp.cols);
        if ci.index != !0 && !is_null(buf.ptr, ci_id) {
          let old = data.add(ci.off as usize);
          let new = buf.ptr.add(ci.off as usize);
          macro_rules! handle {
            ($ty: ident) => {{
              let mut index = Index::<{ $ty }>::new(db, ctx.tp_id, ci_id);
              index.delete(old, rid);
              index.insert(new, rid);
            }};
          }
          handle_all!(ci.ty.fix_ty().ty, handle);
        }
      }
      data.copy_from_nonoverlapping(buf.ptr, slot_size);
      cnt += 1;
      Ok(())
    }, false) { Err(ModifyError(cnt, e)) } else { Ok(cnt) }
  }
}