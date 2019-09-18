#[macro_use]
extern crate static_assertions;

pub mod unsafe_helper;
pub mod ty;
pub mod errors;
pub mod unreachable;

pub use crate::{unsafe_helper::*, errors::*, ty::*};

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

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, hashbrown::hash_map::DefaultHashBuilder>;
pub type IndexSet<K> = indexmap::IndexSet<K, hashbrown::hash_map::DefaultHashBuilder>;
pub type HashMap<K, V> = hashbrown::HashMap<K, V>;
pub type HashSet<K> = hashbrown::HashSet<K>;
pub type HashEntry<'a, K, V> = hashbrown::hash_map::Entry<'a, K, V, hashbrown::hash_map::DefaultHashBuilder>;
pub type IndexEntry<'a, K, V> = indexmap::map::Entry<'a, K, V>;

pub type WithId<T> = (usize, T);