use std::{fmt::Write, path::Path};

use common::{*, Error::*};
use physics::*;
use crate::db::Db;

pub fn show_one_db(path: impl AsRef<Path>, s: &mut String) -> Result<()> {
  let mut db = Db::open(path)?;
  let tables = unsafe { db.get_page::<DbPage>(0).table_num };
  let _ = writeln!(s, "database `{}`: page count = {}, table count = {}", db.path, db.pages, tables);
  Ok(())
}

impl Db {
  pub fn show_table(&self, name: &str) -> Result<String> {
    unsafe {
      let selfm = self.pr();
      let dp = selfm.get_page::<DbPage>(0);
      match dp.names().enumerate().find(|n| n.1 == name) {
        Some((idx, _)) => Ok(selfm.show_table_info(dp.tables.get_unchecked(idx))),
        None => return Err(NoSuchTable(name.into())),
      }
    }
  }

  pub fn show_tables(&self) -> String {
    unsafe {
      let mut s = String::new();
      let selfm = self.pr();
      let dp = selfm.get_page::<DbPage>(0);
      for i in 0..dp.table_num as usize {
        let _ = write!(s, "{}", selfm.show_table_info(dp.tables.get_unchecked(i)));
      }
      s
    }
  }

  unsafe fn show_table_info(&mut self, ti: &TableInfo) -> String {
    let mut s = String::new();
    let tp = self.get_page::<TablePage>(ti.meta as usize);
    let _ = writeln!(s, "table `{}`: meta page = {}, record count = {}",
                     str_from_parts(ti.name.as_ptr(), ti.name_len as usize), ti.meta, tp.count);
    for i in 0..tp.col_num as usize {
      let ci = tp.cols.get_unchecked(i);
      let _ = writeln!(s, "  - col {}: `{}`: {:?} @ offset +{}",
                       i, str_from_parts(ci.name.as_ptr(), ci.name_len as usize), ci.ty, ci.off);
    }
    s
  }
}