use std::{fmt::Write, path::Path};
use unchecked_unwrap::UncheckedUnwrap;

use common::*;
use physics::*;
use crate::db::Db;

pub fn show_db<'a>(path: impl AsRef<Path>, s: &mut String) -> Result<'a, ()> {
  unsafe {
    let mut db = Db::open(path)?;
    let tables = db.dp().table_num;
    writeln!(s, "database `{}`: page count = {}, table count = {}", db.path, db.pages, tables).unchecked_unwrap();
    Ok(())
  }
}

impl Db {
  pub fn show_table<'a>(&self, table: &'a str) -> Result<'a, String> {
    unsafe {
      let tp = self.pr().get_tp(table)?.1;
      let mut s = String::new();
      Self::show_table_info(tp, &mut s);
      Ok(s)
    }
  }

  pub fn show_tables(&self) -> String {
    unsafe {
      let mut s = String::new();
      for &tp_id in self.pr().dp().tables() {
        Self::show_table_info(self.pr().get_page::<TablePage>(tp_id), &mut s);
      }
      s
    }
  }

  unsafe fn show_table_info(tp: &TablePage, s: &mut String) {
    writeln!(s, "table `{}`: record count = {}, record size = {}", tp.name(), tp.count, tp.size).unchecked_unwrap();
    for (idx, ci) in tp.cols().iter().enumerate() {
      writeln!(s, "  - col {}: `{}`: {:?} @ offset +{} ", idx, ci.name(), ci.ty, ci.off).unchecked_unwrap();
      if !ci.flags.is_empty() {
        *s += "    - attr: ";
        if ci.flags.contains(ColFlags::PRIMARY) { *s += "primary + "; }
        if ci.flags.contains(ColFlags::NOTNULL) { *s += "notnull + "; }
        if ci.flags.contains(ColFlags::UNIQUE) { *s += "unique + "; }
        (s.pop(), s.pop());
        s.push('\n');
      }
      if let Some(idx) = ci.idx_name() {
        *s += "    - index: ";
        if idx.is_empty() { *s += "<internal>"; } else { write!(s, "`{}`", idx).unchecked_unwrap(); }
        s.push('\n');
      }
    }
  }
}