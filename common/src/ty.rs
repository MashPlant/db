use std::{fmt, cmp::Ordering, mem, marker::PhantomData};
use unchecked_unwrap::UncheckedUnwrap;
use chrono::NaiveDate;
use crate::debug_unreachable;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BareTy { Bool, Int, Float, Date, VarChar }

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
    match self.ty { Bool => 1, Int | Float => 4, Date => 4, VarChar => self.size as u16 + 1 }
  }

  pub fn align4(self) -> bool {
    use BareTy::*;
    match self.ty { Bool | VarChar => false, Int | Float | Date => true }
  }
}

// `Date` can not be produced by parser, but can be used to pass the result of select
#[derive(Copy, Clone)]
pub enum Lit<'a> { Null, Bool(bool), Number(f64), Date(NaiveDate), Str(&'a str) }

// the discriminant of Lit
#[derive(Debug)]
pub enum LitTy { Null, Bool, Number, Date, Str }

impl Lit<'_> {
  pub fn is_null(&self) -> bool { match self { Lit::Null => true, _ => false, } }

  pub fn ty(&self) -> LitTy {
    use Lit::*;
    match self { Null => LitTy::Null, Bool(_) => LitTy::Bool, Number(_) => LitTy::Number, Date(_) => LitTy::Date, Str(_) => LitTy::Str }
  }

  // only accept the same variant to compare,
  pub unsafe fn cmp(&self, other: &Lit) -> Ordering {
    match (self, other) {
      (Lit::Null, Lit::Null) => Ordering::Equal,
      (Lit::Bool(l), Lit::Bool(r)) => l.cmp(r),
      // it is okay to unwrap, because we never get NaN in Lit
      (Lit::Number(l), Lit::Number(r)) => l.partial_cmp(r).unchecked_unwrap(),
      (Lit::Date(l), Lit::Date(r)) => l.cmp(r),
      (Lit::Str(l), Lit::Str(r)) => l.cmp(r),
      _ => debug_unreachable!(),
    }
  }
}

impl fmt::Debug for ColTy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}({})", self.ty, self.size) }
}

impl fmt::Debug for Lit<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use Lit::*;
    match *self {
      Null => write!(f, "null"), Bool(x) => write!(f, "{}", x), Number(x) => write!(f, "{}", x),
      Date(x) => write!(f, "{}", x), Str(x) => write!(f, "'{}'", x)
    }
  }
}

// C for Compressed: Lit takes 24 bytes of space, which is not efficient enough
// Lit is used in functions, CLit is used in data structures
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct CLit<'a>((u64, u64, PhantomData<&'a str>));

impl<'a> CLit<'a> {
  // I don't expect it to work on a 32-bit system
  #[cfg_attr(tarpaulin, skip)]
  fn _ck() { assert_eq_size!(u64, usize); }

  pub fn new(lit: Lit<'a>) -> Self {
    unsafe {
      match lit {
        Lit::Null => Self((0, 0, PhantomData)),
        Lit::Bool(x) => Self((1, x as u64, PhantomData)),
        Lit::Number(x) => Self((2, mem::transmute(x), PhantomData)),
        Lit::Date(x) => Self((3, mem::transmute::<_, u32>(x) as u64, PhantomData)),
        Lit::Str(x) => Self(mem::transmute(x)),
      }
    }
  }

  pub fn lit(self) -> Lit<'a> {
    unsafe {
      let data = self.0;
      match data.0 {
        0 => Lit::Null,
        1 => Lit::Bool(data.1 != 0),
        2 => Lit::Number(mem::transmute(data.1)),
        3 => Lit::Date(mem::transmute(data.1 as u32)),
        _ => Lit::Str(mem::transmute(data))
      }
    }
  }

  pub fn is_null(self) -> bool { (self.0).0 == 0 }

  pub unsafe fn cmp(self, other: CLit) -> Ordering { self.lit().cmp(&other.lit()) }
}

impl fmt::Debug for CLit<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> { write!(f, "{:?}", self.lit()) }
}

// Agg, Sum is available for Int, Float
// Min, Max, Count is available for all
// CountAll is special, it comes from count(*), so it doesn't have ColRef
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AggOp { Avg, Sum, Min, Max, Count, CountAll }

impl AggOp {
  pub fn name(self) -> &'static str {
    match self { AggOp::Avg => "avg", AggOp::Sum => "sum", AggOp::Min => "min", AggOp::Max => "max", AggOp::Count | AggOp::CountAll => "count" }
  }
}

#[derive(Debug, Copy, Clone)]
pub enum BinOp { Add, Sub, Mul, Div, Mod }

impl BinOp {
  pub fn name(self) -> char {
    match self { BinOp::Add => '+', BinOp::Sub => '-', BinOp::Mul => '*', BinOp::Div => '/', BinOp::Mod => '%' }
  }
}