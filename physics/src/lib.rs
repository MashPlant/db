#![feature(ptr_offset_from)]
#[macro_use]
extern crate static_assertions;

pub mod data_page;
pub mod db_page;
pub mod index_page;
pub mod table_page;
pub mod rid;

pub use crate::{data_page::*, db_page::*, index_page::*, table_page::*, rid::*};