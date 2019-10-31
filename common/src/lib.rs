#![feature(ptr_offset_from)]
#[macro_use]
extern crate static_assertions;

pub mod unsafe_helper;
pub mod ty;
pub mod errors;

pub use crate::{unsafe_helper::*, errors::*, ty::*};

pub const MAGIC_LEN: usize = 18;
pub const MAGIC: &[u8; MAGIC_LEN] = b"MashPlant-DataBase";
pub const LOB_SUFFIX: &str = "lob";
pub const LOG_MAX_SLOT: usize = 9;
pub const MAX_PAGE: usize = 1 << (32 - LOG_MAX_SLOT);
pub const MAX_SLOT: usize = 1 << LOG_MAX_SLOT; // 512 (actually can hold up to MAX_DATA_BYTE / MIN_SLOT_SIZE = 507)
pub const MAX_SLOT_BS: usize = MAX_SLOT / 32; // 16
pub const MIN_SLOT_SIZE: usize = PAGE_SIZE / MAX_SLOT; // 16
pub const MAX_DATA_BYTE: usize = PAGE_SIZE - 12 - MAX_SLOT_BS * 4; // 8116 (12 is the size of all other fields in DataPage)
pub const PAGE_SIZE: usize = 8192;
pub const VARCHAR_SLOT_SIZE: usize = 8; // see physics::VarcharSlot (this is how Varchar info is stored in data slot, not how Varchar data is stored as lob)

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, hashbrown::hash_map::DefaultHashBuilder>;
pub type IndexSet<K> = indexmap::IndexSet<K, hashbrown::hash_map::DefaultHashBuilder>;
pub type HashMap<K, V> = hashbrown::HashMap<K, V>;
pub type HashSet<K> = hashbrown::HashSet<K>;

// save some typing
#[macro_use]
mod macros {
  #[macro_export] macro_rules! bool { () => { ColTy::FixTy(FixTy { ty: Bool, .. }) }; }
  #[macro_export] macro_rules! int { () => { ColTy::FixTy(FixTy { ty: Int, .. }) }; }
  #[macro_export] macro_rules! float { () => { ColTy::FixTy(FixTy { ty: Float, .. }) }; }
  #[macro_export] macro_rules! date { () => { ColTy::FixTy(FixTy { ty: Date, .. }) }; }
  #[macro_export] macro_rules! char {
    () => { ColTy::FixTy(FixTy { ty: Char, .. }) };
    ($size: ident) => { ColTy::FixTy(FixTy { ty: Char, size: $size }) };
  }
  #[macro_export] macro_rules! varchar {
    () => { ColTy::Varchar(_) };
    ($size: ident) => { ColTy::Varchar($size) };
  }
  #[macro_export] macro_rules! impossible {
    () => (if cfg!(debug_assertions) { unreachable!(); } else { std::hint::unreachable_unchecked() })
  }
}