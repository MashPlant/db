use std::fmt;
use chrono::NaiveDate;

use common::{*, BareTy::*, Error::*};
use syntax::ast::*;
use physics::*;
use index::Index;
use db::Db;

pub struct SelectResult {}

impl fmt::Display for SelectResult {
  fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
    unimplemented!()
  }
}

pub fn select(s: &Select, _db: &mut Db) -> Result<SelectResult> {
  debug_assert!(s.tables.len() >= 1);
  if s.tables.len() == 1 {} else {}
  unimplemented!()
}
