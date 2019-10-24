use std::{fmt::Write, path::Path};
use unchecked_unwrap::UncheckedUnwrap;

use common::*;
use physics::*;
use crate::{db::Db, ptr2lit};

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
      self.show_table_info(tp, &mut s);
      Ok((s.pop(), s).1)
    }
  }

  pub fn show_tables(&self) -> String {
    unsafe {
      let mut s = String::new();
      for &tp_id in self.pr().dp().tables() {
        self.show_table_info(self.pr().get_page::<TablePage>(tp_id), &mut s);
      }
      (s.pop(), s).1
    }
  }

  unsafe fn show_table_info(&self, tp: &TablePage, s: &mut String) {
    writeln!(s, "table `{}`: record count = {}, record size = {}", tp.name(), tp.count, tp.size).unchecked_unwrap();
    for (idx, ci) in tp.cols().iter().enumerate() {
      writeln!(s, "  - col {}: `{}`: {:?} @ offset +{} ", idx, ci.name(), ci.ty, ci.off).unchecked_unwrap();
      if !ci.flags.is_empty() {
        *s += "    - attr: ";
        if ci.flags.contains(ColFlags::PRIMARY) { *s += "primary + "; }
        if ci.flags.contains(ColFlags::NOTNULL) { *s += "notnull + "; }
        if ci.flags.contains(ColFlags::UNIQUE) { *s += "unique + "; }
        (s.pop(), s.pop(), s.push('\n'));
      }
      if ci.f_table != !0 {
        let f_tp = self.pr().get_page::<TablePage>(ci.f_table);
        let f_ci = f_tp.cols.get_unchecked(ci.f_col as usize);
        writeln!(s, "    - foreign: `{}.{}`", f_tp.name(), f_ci.name()).unchecked_unwrap();
      }
      if let Some(idx) = ci.idx_name() {
        *s += "    - index: ";
        if idx.is_empty() { *s += "<internal>"; } else { write!(s, "`{}`", idx).unchecked_unwrap(); }
        s.push('\n');
      }
      if ci.check != !0 {
        let cp = self.pr().get_page::<CheckPage>(ci.check >> 1);
        let (count, size) = (cp.count as usize, ci.ty.size() as usize);
        if count != 0 {
          *s += "    - check: ";
          for idx in 0..count {
            write!(s, "{:?}, ", ptr2lit(cp.data.as_ptr().add(idx * size), ci.ty.ty)).unchecked_unwrap();
          }
          (s.pop(), s.pop(), s.push('\n'));
        }
        if (ci.check & 1) == 1 {
          writeln!(s, "    - default: {:?}", ptr2lit(cp.data.as_ptr().add(count * size), ci.ty.ty)).unchecked_unwrap();
        }
      }
    }
  }
}