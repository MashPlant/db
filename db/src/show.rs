use std::{fmt::Write, path::Path};
use unchecked_unwrap::UncheckedUnwrap;

use common::{*, Error::*};
use physics::*;
use crate::db::Db;

pub fn show_db(path: impl AsRef<Path>, s: &mut String) -> Result<()> {
  unsafe {
    let mut db = Db::open(path)?;
    let tables = db.get_page::<DbPage>(0).table_num;
    writeln!(s, "database `{}`: page count = {}, table count = {}", db.path, db.pages, tables).unchecked_unwrap();
    Ok(())
  }
}

impl Db {
  pub fn show_table(&self, name: &str) -> Result<String> {
    unsafe {
      let selfm = self.pr();
      let dp = selfm.get_page::<DbPage>(0);
      match dp.names().enumerate().find(|n| n.1 == name) {
        Some((idx, _)) => {
          let mut s = String::new();
          selfm.show_table_info(dp.tables.get_unchecked(idx), &mut s);
          Ok(s)
        }
        None => return Err(NoSuchTable(name.into())),
      }
    }
  }

  pub fn show_tables(&self) -> String {
    unsafe {
      let mut s = String::new();
      let selfm = self.pr();
      let dp = selfm.get_page::<DbPage>(0);
      for ti in dp.tables() { selfm.show_table_info(ti, &mut s); }
      s
    }
  }

  unsafe fn show_table_info(&mut self, ti: &TableInfo, s: &mut String) {
    let tp = self.get_page::<TablePage>(ti.meta as usize);
    writeln!(s, "table `{}`: record count = {}, record size = {}",
                     str_from_parts(ti.name.as_ptr(), ti.name_len as usize), tp.count, tp.size).unchecked_unwrap();
    for (idx, ci) in tp.cols().iter().enumerate() {
      write!(s, "  - col {}: `{}`: {:?} @ offset +{}; ", idx, str_from_parts(ci.name.as_ptr(), ci.name_len as usize), ci.ty, ci.off).unchecked_unwrap();
      if ci.flags.contains(ColFlags::PRIMARY) { s.push_str("primary "); }
      if ci.flags.contains(ColFlags::NOTNULL) { s.push_str("notnull "); }
      if ci.flags.contains(ColFlags::UNIQUE) { s.push_str("unique "); }
      s.push('\n');
    }
  }
}