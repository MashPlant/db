#[macro_use]
extern crate static_assertions;

use std::{str, slice, alloc::{alloc, dealloc, Layout}};

pub mod errors;
pub mod unreachable;

pub use crate::errors::*;

pub const MAGIC_LEN: usize = 18;
pub const MAGIC: &[u8; MAGIC_LEN] = b"MashPlant-DataBase";
pub const LOG_MAX_SLOT: usize = 9;
pub const MAX_PAGE: usize = 1 << (32 - LOG_MAX_SLOT);
// actually can hold up to MAX_DATA_BYTE / MIN_SLOT_SIZE = 507
pub const MAX_SLOT: usize = 1 << LOG_MAX_SLOT;
pub const MAX_SLOT_BS: usize = MAX_SLOT / 32;
pub const MIN_SLOT_SIZE: usize = PAGE_SIZE / MAX_SLOT;
pub const MAX_DATA_BYTE: usize = PAGE_SIZE - (4 + MAX_SLOT_BS) * 4 /* = 8112 */;
pub const PAGE_SIZE: usize = 8192;

pub trait Ptr2Ref {
  type Target;
  unsafe fn r<'a>(self) -> &'a mut Self::Target;
}

pub trait Ref2PtrMut {
  type Target;
  fn p(self) -> *mut Self::Target;

  // pr for p().r()
  #[inline(always)]
  unsafe fn pr<'a>(self) -> &'a mut Self::Target where Self: std::marker::Sized { self.p().r() }

  // prc for const version of pr
  #[inline(always)]
  unsafe fn prc<'a>(self) -> &'a Self::Target where Self: std::marker::Sized { &*self.p().r() }
}

impl<T> Ptr2Ref for *mut T {
  type Target = T;
  #[inline(always)]
  unsafe fn r<'a>(self) -> &'a mut T { &mut *self }
}

// like const cast
impl<T> Ptr2Ref for *const T {
  type Target = T;
  #[inline(always)]
  unsafe fn r<'a>(self) -> &'a mut T { &mut *(self as *mut T) }
}

impl<T> Ref2PtrMut for &mut T {
  type Target = T;
  #[inline(always)]
  fn p(self) -> *mut T { self as *mut T }
}

impl<T> Ref2PtrMut for &T {
  type Target = T;
  #[inline(always)]
  fn p(self) -> *mut T { self as *const T as *mut T }
}

#[inline(always)]
pub unsafe fn str_from_parts<'a>(data: *const u8, len: usize) -> &'a str {
  str::from_utf8_unchecked(slice::from_raw_parts(data, len))
}

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

pub struct Align4U8 {
  pub ptr: *mut u8,
  pub size: usize,
}

impl Align4U8 {
  pub unsafe fn new(size: usize) -> Align4U8 {
    Align4U8 { ptr: alloc(Layout::from_size_align_unchecked(size, 4)), size }
  }
}

impl Drop for Align4U8 {
  fn drop(&mut self) {
    unsafe { dealloc(self.ptr, Layout::from_size_align_unchecked(self.size, 4)) }
  }
}

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, hashbrown::hash_map::DefaultHashBuilder>;
pub type IndexSet<K> = indexmap::IndexSet<K, hashbrown::hash_map::DefaultHashBuilder>;
pub type HashMap<K, V> = hashbrown::HashMap<K, V>;
pub type HashSet<K> = hashbrown::HashSet<K>;
pub type HashEntry<'a, K, V> = hashbrown::hash_map::Entry<'a, K, V, hashbrown::hash_map::DefaultHashBuilder>;
pub type IndexEntry<'a, K, V> = indexmap::map::Entry<'a, K, V>;