use unchecked_unwrap::UncheckedUnwrap;
use regex::Regex;
use std::cmp::Ordering::*;

use common::{*, Error::*, BinOp::*, CmpOp::*, BareTy::*};
use syntax::ast::*;
use physics::*;
use db::{Db, fill_ptr, ptr2lit};
use index::{Index, handle_all};
use crate::{is_null, predicate::one_where, filter::filter, InsertCtx};

unsafe fn check<'a>(e: &Expr<'a>, tp: &mut TablePage, re_cache: &mut HashMap<&'a str, Regex>) -> Result<'a, LitTy> {
  match e {
    Expr::Atom(x) => Ok(match x {
      Atom::Lit(x) => x.lit().ty(),
      Atom::ColRef(col) => {
        if let Some(t) = col.table { if t != tp.name() { return Err(NoSuchTable(t)); } }
        let ci = tp.get_ci(col.col)?;
        match ci.ty.ty { Bool => LitTy::Bool, Int | Float => LitTy::Number, Date => LitTy::Date, VarChar => LitTy::Str }
      }
    }),
    Expr::Null(x, _) => {
      check(x, tp, re_cache)?;
      Ok(LitTy::Bool)
    }
    Expr::Like(x, like) => {
      match check(x, tp, re_cache)? { LitTy::Str => {} ty => return Err(InvalidLikeTy1(ty)) };
      re_cache.insert(like, db::like2re(like)?);
      Ok(LitTy::Bool)
    }
    Expr::Neg(x) => {
      match check(x, tp, re_cache)? { LitTy::Number => {} ty => return Err(IncompatibleBin { op: Sub, ty }) };
      Ok(LitTy::Number)
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
// if div0 or mod0 occurs, the result is null (that is how sqlite behaves)
unsafe fn eval<'a>(e: &Expr<'a>, tp: &mut TablePage, data: *const u8, re_cache: &HashMap<&'a str, Regex>) -> Lit<'a> {
  match e {
    Expr::Atom(x) => match x {
      Atom::Lit(x) => *x,
      Atom::ColRef(col) => {
        let ci = tp.get_ci(col.col).unchecked_unwrap();
        let ci_id = ci.idx(&tp.cols);
        ptr2lit(data, ci_id, ci)
      }
    }.lit(),
    Expr::Null(x, null) => {
      let x = eval(x, tp, data, re_cache);
      Lit::Bool(x.is_null() == *null)
    }
    Expr::Like(x, like) => {
      let re = re_cache.get(like).unchecked_unwrap();
      let x = match eval(x, tp, data, re_cache) { Lit::Str(x) => x, _ => return Lit::Null };
      Lit::Bool(re.is_match(x))
    }
    Expr::Neg(x) => {
      // since we cannot have type mismatch here, if it is not Number, it can only be Null
      let x = match eval(x, tp, data, re_cache) { Lit::Number(x) => x, _ => return Lit::Null };
      Lit::Number(-x)
    }
    Expr::And(box (l, r)) | Expr::Or(box (l, r)) => {
      let or = if let Expr::Or(_) = e { true } else { false };
      let l = match eval(l, tp, data, re_cache) { Lit::Bool(x) => x, _ => return Lit::Null };
      if or == l { return Lit::Bool(l); } // short circuit, true or _ / false and _
      // now it is false or _ / true and _, the result only depends on `r`
      let r = match eval(r, tp, data, re_cache) { Lit::Bool(x) => x, _ => return Lit::Null };
      Lit::Bool(r)
    }
    Expr::Cmp(op, box (l, r)) => {
      let l = eval(l, tp, data, re_cache);
      let r = eval(r, tp, data, re_cache);
      if l.is_null() || r.is_null() { return Lit::Null; };
      let cmp = l.cmp(&r); // `check` and null check above guarantees they have the same type
      Lit::Bool(match op { Lt => cmp == Less, Le => cmp != Greater, Ge => cmp != Less, Gt => cmp == Greater, Eq => cmp == Equal, Ne => cmp != Equal })
    }
    Expr::Bin(op, box (l, r)) => {
      let l = match eval(l, tp, data, re_cache) { Lit::Number(x) => x, _ => return Lit::Null };
      let r = match eval(r, tp, data, re_cache) { Lit::Number(x) => x, _ => return Lit::Null };
      Lit::Number(match op {
        Add => l + r, Sub => l - r, Mul => l * r,
        Div => if r == 0.0 { return Lit::Null; } else { l / r }, Mod => if r == 0.0 { return Lit::Null; } else { l % r },
      })
    }
  }
}

pub fn update<'a>(u: &Update<'a>, db: &mut Db) -> Result<'a, String> {
  unsafe {
    let mut ctx = InsertCtx::build(db, u.table)?;
    let pred = one_where(&u.where_, ctx.tp)?;
    if db.has_foreign_link_to(ctx.tp_id) { return Err(AlterTableWithForeignLink(u.table)); }
    let mut re_cache = HashMap::new();
    for (col, e) in &u.sets {
      ctx.tp.get_ci(col)?;
      check(e, ctx.tp, &mut re_cache)?;
    }
    let slot_size = ctx.tp.size as usize;
    let buf = Align4U8::new(slot_size); // update to buf, then copy to db
    let mut update_num = 0u32;
    filter(db.pr(), &u.where_, ctx.tp_id, ctx.tp.pr(), pred, |data, rid| {
      update_num += 1;
      buf.ptr.copy_from_nonoverlapping(data, slot_size);
      for (col, e) in &u.sets {
        let ci = ctx.tp.get_ci(col).unchecked_unwrap();
        let ci_id = ci.idx(&ctx.tp.cols);
        let val = CLit::new(eval(e, ctx.tp, data, &re_cache));
        if val.is_null() {
          if ci.flags.contains(ColFlags::NOTNULL) { return Err(PutNullOnNotNull); }
          bsset(buf.ptr as *mut u32, ci_id as usize);
        } else {
          bsdel(buf.ptr as *mut u32, ci_id as usize); // now not null (no matter whether it is null before)
          fill_ptr(buf.ptr.add(ci.off as usize), ci.ty, val)?;
        }
        ctx.check_col(buf.ptr, ci_id, val, Some(rid))?; // it won't conflict with the old value (`data`)
      }
      if ctx.pks.len() > 1 {
        let old = ctx.pk_set.remove(&InsertCtx::hash_pks(data, &ctx.pks));
        debug_assert!(old); // the old hash should exist, if implementation is correct
        if !ctx.pk_set.insert(InsertCtx::hash_pks(buf.ptr, &ctx.pks)) { return Err(PutDupCompositePrimaryKey); }
      }
      // now no error can occur
      for (col, _) in &u.sets {
        let ci = ctx.tp.get_ci(col).unchecked_unwrap();
        let ci_id = ci.idx(&ctx.tp.cols);
        if ci.index != !0 && !is_null(buf.ptr, ci_id) {
          let old = data.add(ci.off as usize);
          let new = buf.ptr.add(ci.off as usize);
          macro_rules! handle {
            ($ty: ident) => {{
              let mut index = Index::<{ $ty }>::new(db, Rid::new(ctx.tp_id, ci_id));
              index.delete(old, rid);
              index.insert(new, rid);
            }};
          }
          handle_all!(ci.ty.ty, handle);
        }
      }
      data.copy_from_nonoverlapping(buf.ptr, slot_size);
      Ok(())
    }, false)?;
    Ok(format!("{} record(s) updated", update_num))
  }
}