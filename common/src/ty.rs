use std::{fmt, cmp::Ordering};
use unchecked_unwrap::UncheckedUnwrap;
use chrono::NaiveDate;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BareTy { Int, Bool, Float, VarChar, Date }

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ColTy {
  pub ty: BareTy,
  pub size: u8,
}

impl ColTy {
  #[cfg_attr(tarpaulin, skip)]
  fn _ck() {
    assert_eq_size!(ColTy, u16);
    assert_eq_size!(chrono::NaiveDate, u32);
  }

  // char and varchar can have size = 255 + 1, so u16 is necessary
  pub fn size(self) -> u16 {
    use BareTy::*;
    match self.ty { Int => 4, Bool => 1, Float => 4, VarChar => self.size as u16 + 1, Date => 4 }
  }

  pub fn align4(self) -> bool {
    use BareTy::*;
    match self.ty { Int | Float | Date => true, Bool | VarChar => false }
  }
}

#[derive(Copy, Clone)]
pub enum Lit<'a> { Null, Int(i32), Bool(bool), Float(f32), Str(&'a str) }

// add f64 and date, these 2 cannot be produced by parser
// LitExt is to pass result of `select`
#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub enum LitExt<'a> { Null, Int(i32), Bool(bool), Float(f32), Str(&'a str), Date(NaiveDate), F64(f64) }

#[derive(Debug)]
pub enum LitTy { Null, Int, Bool, Float, Str }

impl Lit<'_> {
  pub fn is_null(&self) -> bool { match self { Lit::Null => true, _ => false, } }

  pub fn ty(&self) -> LitTy {
    use Lit::*;
    match self { Null => LitTy::Null, Int(_) => LitTy::Int, Bool(_) => LitTy::Bool, Float(_) => LitTy::Float, Str(_) => LitTy::Str, }
  }
}

// these 2 traits must be implemented manually because Lit contains f32
// it is okay to implement them, because NaN can never appear in LitExt
impl Eq for LitExt<'_> {}

impl Ord for LitExt<'_> {
  fn cmp(&self, other: &Self) -> Ordering {
    unsafe { self.partial_cmp(other).unchecked_unwrap() }
  }
}

impl fmt::Debug for ColTy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}({})", self.ty, self.size) }
}

impl fmt::Debug for Lit<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use Lit::*;
    match self {
      Null => write!(f, "null"), Int(x) => write!(f, "{}", x), Bool(x) => write!(f, "{}", x),
      Float(x) => write!(f, "{}f", x), Str(x) => write!(f, "'{}'", x)
    }
  }
}

// some tiny modifications comparing to Lit, because this is output to a csv file
impl fmt::Debug for LitExt<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use LitExt::*;
    match self {
      Null => Ok(()), Int(x) => write!(f, "{}", x), Bool(x) => write!(f, "{}", x),
      Float(x) => write!(f, "{}", x), Str(x) => write!(f, "{}", x),
      Date(x) => write!(f, "{}", x), F64(x) => write!(f, "{}", x)
    }
  }
}

// Agg, Sum is available for Int, Bool, Float
// Min, Max, Count is available for all
// CountAll is special, it comes from count(*), so it doesn't have ColRef
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AggOp { Avg, Sum, Min, Max, Count, CountAll }

impl AggOp {
  pub fn name(self) -> &'static str {
    use AggOp::*;
    match self { Avg => "avg", Sum => "sum", Min => "min", Max => "max", Count | CountAll => "count" }
  }
}