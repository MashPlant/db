use std::{str, slice, alloc::{alloc, dealloc, Layout}};

pub trait Ptr2Ref {
  type Target;
  unsafe fn r<'a>(self) -> &'a mut Self::Target;
}

pub trait Ref2PtrMut where Self: Sized {
  type Target;
  fn p(self) -> *mut Self::Target;

  unsafe fn idx(self, arr: &[Self::Target]) -> u32 { (self.p()).offset_from(arr.as_ptr()) as u32 }
  // pr for p().r()
  unsafe fn pr<'a>(self) -> &'a mut Self::Target where Self: std::marker::Sized { self.p().r() }
}

impl<T> Ptr2Ref for *mut T {
  type Target = T;
  unsafe fn r<'a>(self) -> &'a mut T { &mut *self }
}

// like const cast
impl<T> Ptr2Ref for *const T {
  type Target = T;
  unsafe fn r<'a>(self) -> &'a mut T { &mut *(self as *mut T) }
}

impl<T> Ref2PtrMut for &mut T {
  type Target = T;
  fn p(self) -> *mut T { self as *mut T }
}

impl<T> Ref2PtrMut for &T {
  type Target = T;
  fn p(self) -> *mut T { self as *const T as *mut T }
}

pub unsafe fn str_from_parts<'a>(data: *const u8, len: usize) -> &'a str {
  str::from_utf8_unchecked(slice::from_raw_parts(data, len))
}

// add str in db records are stored in this way, ptr[0] = len (at most 255), ptr[1..] = contents
pub unsafe fn str_from_db<'a>(ptr: *const u8) -> &'a str {
  str_from_parts(ptr.add(1), *ptr as usize)
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

pub unsafe fn bsget(p: *const u32, idx: usize) -> bool { ((*p.add(idx / 32) >> (idx % 32)) & 1) != 0 }

pub unsafe fn bsset(p: *mut u32, idx: usize) { *p.add(idx / 32) |= 1 << (idx % 32); }

pub unsafe fn bsdel(p: *mut u32, idx: usize) { *p.add(idx / 32) &= !(1 << (idx % 32)); }