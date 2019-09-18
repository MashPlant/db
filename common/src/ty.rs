
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BareTy { Int, Bool, Float, Char, VarChar, Date }

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
  #[inline(always)]
  pub fn size(self) -> u16 {
    use BareTy::*;
    match self.ty { Int => 4, Bool => 1, Float => 4, Char | VarChar => self.size as u16 + 1, Date => 4 }
  }

  #[inline(always)]
  pub fn align4(self) -> bool {
    use BareTy::*;
    match self.ty { Int | Float | Date => true, Bool | Char | VarChar => false }
  }
}

#[derive(Debug, Copy, Clone)]
pub enum Lit<'a> { Null, Int(i32), Bool(bool), Float(f32), Str(&'a str) }

#[derive(Debug)]
pub enum OwnedLit { Null, Int(i32), Bool(bool), Float(f32), Str(Box<str>) }

#[derive(Debug)]
pub enum LitTy { Null, Int, Bool, Float, Str }

impl Lit<'_> {
  pub fn is_null(&self) -> bool { match self { Lit::Null => true, _ => false, } }

  pub fn ty(&self) -> LitTy {
    use Lit::*;
    match self { Null => LitTy::Null, Int(_) => LitTy::Int, Bool(_) => LitTy::Bool, Float(_) => LitTy::Float, Str(_) => LitTy::Str, }
  }

  pub fn to_owned(&self) -> OwnedLit {
    use Lit::*;
    match *self { Null => OwnedLit::Null, Int(v) => OwnedLit::Int(v), Bool(v) => OwnedLit::Bool(v), Float(v) => OwnedLit::Float(v), Str(v) => OwnedLit::Str(v.into()), }
  }
}