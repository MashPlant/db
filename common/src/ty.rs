use std::{fmt, cmp::Ordering, mem, marker::PhantomData};
use chrono::NaiveDate;
use crate::{impossible, varchar, VARCHAR_SLOT_SIZE};

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BareTy { Bool, Int, Float, Date, Char }

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct FixTy {
  pub ty: BareTy,
  pub size: u8,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColTy { FixTy(FixTy), Varchar(u16) }

impl ColTy {
  #[cfg_attr(tarpaulin, skip)]
  fn _ck() {
    assert_eq_size!(FixTy, u16);
    assert_eq_size!(ColTy, u32);
    assert_eq_size!(NaiveDate, u32);
  }

  pub fn is_varchar(self) -> bool { match self { ColTy::FixTy(_) => false, varchar!() => true } }

  // guarantee: !self.is_varchar() <=> self.fix_ty() is safe
  pub unsafe fn fix_ty(self) -> FixTy { match self { ColTy::FixTy(x) => x, varchar!() => impossible!() } }

  // char and varchar can have size = 255 + 1, so u16 is necessary
  pub fn size(self) -> u16 {
    use BareTy::*;
    match self {
      ColTy::FixTy(ty) => match ty.ty { Bool => 1, Int | Float => 4, Date => 4, Char => ty.size as u16 + 1 }
      varchar!() => VARCHAR_SLOT_SIZE as u16,
    }
  }

  pub fn align4(self) -> bool {
    use BareTy::*;
    match self {
      ColTy::FixTy(ty) => match ty.ty { Bool | Char => false, Int | Float | Date => true }
      varchar!() => true,
    }
  }
}

// `Date` can not be produced by parser, but can be used to pass the result of select
#[derive(Copy, Clone)]
pub enum Lit<'a> { Null, Bool(bool), Number(f64), Date(NaiveDate), Str(&'a str) }

// the discriminant of Lit
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
      (&Lit::Number(l), &Lit::Number(r)) => fcmp(l, r),
      (Lit::Date(l), Lit::Date(r)) => l.cmp(r),
      (Lit::Str(l), Lit::Str(r)) => l.cmp(r),
      _ => impossible!(),
    }
  }
}

// this can be used for the comparison between non-nan float (in the database we always guarantee float is not-nan)
// why not using partial_cmp + unchecked_unwrap? I've check the output asm, this way is more efficient
pub fn fcmp<T: PartialOrd>(l: T, r: T) -> Ordering {
  if l < r { Ordering::Less } else if l > r { Ordering::Greater } else { Ordering::Equal }
}

impl fmt::Debug for FixTy {
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
// Lit is used in functions to implement logic, CLit is used in data structures to save space
#[derive(Copy, Clone)]
pub struct CLit<'a>(u64, u64, PhantomData<&'a str>);

impl<'a> CLit<'a> {
  // I don't expect it to work on a 32-bit system
  #[cfg_attr(tarpaulin, skip)]
  fn _ck() { assert_eq_size!(u64, usize); }

  pub fn new(lit: Lit<'a>) -> Self {
    unsafe {
      match lit {
        Lit::Null => Self(0, 0, PhantomData),
        Lit::Bool(x) => Self(1, x as u64, PhantomData),
        Lit::Number(x) => Self(2, mem::transmute(x), PhantomData),
        Lit::Date(x) => Self(3, mem::transmute::<_, u32>(x) as u64, PhantomData),
        Lit::Str(x) => mem::transmute(x),
      }
    }
  }

  pub fn lit(self) -> Lit<'a> {
    unsafe {
      match self.0 {
        0 => Lit::Null,
        1 => Lit::Bool(self.1 != 0),
        2 => Lit::Number(mem::transmute(self.1)),
        3 => Lit::Date(mem::transmute(self.1 as u32)),
        _ => Lit::Str(mem::transmute(self))
      }
    }
  }

  pub fn is_null(self) -> bool { self.0 == 0 }

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
    use AggOp::*;
    match self { Avg => "avg", Sum => "sum", Min => "min", Max => "max", Count | CountAll => "count" }
  }
}

#[derive(Debug, Copy, Clone)]
pub enum BinOp { Add, Sub, Mul, Div, Mod }

impl BinOp {
  pub fn name(self) -> char {
    use BinOp::*;
    match self { Add => '+', Sub => '-', Mul => '*', Div => '/', Mod => '%' }
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CmpOp { Lt, Le, Ge, Gt, Eq, Ne }

impl CmpOp {
  pub fn name(self) -> &'static str {
    use CmpOp::*;
    match self { Lt => "<", Le => "<=", Ge => ">=", Gt => ">", Eq => "==", Ne => "!=" }
  }

  // (x <op> y) == (y <op.rev()> x)
  pub fn rev(self) -> CmpOp {
    use CmpOp::*;
    match self { Lt => Gt, Le => Ge, Ge => Le, Gt => Lt, Eq => Eq, Ne => Ne }
  }
}