use common::{*, Error::*};
use physics::*;
use crate::{Db, is_null};

impl Db {
  // only alloc one index page for ci, records are not inserted into index (this is done by `index` crate)
  // `index` may be an empty string, this means it is an internal index (no extra operation needed)
  pub unsafe fn alloc_index<'a>(&mut self, ci: &mut ColInfo, index: &'a str) -> Result<'a, ()> {
    debug_assert!(!ci.ty.is_varchar());
    if index.len() > MAX_IDX_NAME { return Err(IndexNameTooLong(index)); }
    ci.idx_name_len = index.len() as u8;
    ci.idx_name.as_mut_ptr().copy_from_nonoverlapping(index.as_ptr(), index.len());
    let (id, ip) = self.alloc_page::<IndexPage>();
    ci.index = id;
    ip.init(true, ci.ty.size()); // it is the root, but also a leaf
    Ok(())
  }

  pub fn drop_index<'a>(&mut self, index: &'a str, table: Option<&'a str>) -> Result<'a, ()> {
    unsafe {
      for &tp_id in self.dp().tables() {
        let tp = self.get_page::<TablePage>(tp_id);
        for ci in tp.cols() {
          if ci.idx_name().filter(|&x| !x.is_empty() && x == index).is_some() {
            // `table` is only for error checking
            match table { Some(t) if t != tp.name() => return Err(NoSuchIndex(index)), _ => {} };
            self.dealloc_index(ci.index);
            ci.pr().index = !0;
            return Ok(());
          }
        }
      }
      return Err(NoSuchIndex(index));
    }
  }

  // only deallocate index pages, ColInfo::index is not affected
  pub unsafe fn dealloc_index(&mut self, root: u32) {
    unsafe fn dfs(db: &mut Db, page: u32) {
      let ip = db.get_page::<IndexPage>(page);
      let (slot_size, key_size) = (ip.slot_size() as usize, ip.key_size() as usize);
      macro_rules! at_ch { ($pos: expr) => { *(ip.data.as_mut_ptr().add($pos * slot_size + key_size) as *mut u32) }; }
      if !ip.leaf { for i in 0..ip.count as usize { dfs(db, at_ch!(i)); } }
      db.dealloc_page(page);
    }
    dfs(self, root);
  }
}

impl Db {
  // unfortunately we don't know whether the index introduced by foreign constraint can be dropped or not, so just leave it here
  pub fn drop_foreign<'a>(&mut self, table: &'a str, col: &'a str) -> Result<'a, ()> {
    unsafe {
      let ci = self.get_tp(table)?.1.get_ci(col)?;
      if ci.f_table == !0 { return Err(NoSuchForeign(col)); }
      ci.f_table = !0;
      Ok(())
    }
  }

  pub fn rename_table<'a>(&mut self, old: &'a str, new: &'a str) -> Result<'a, ()> {
    unsafe {
      let tp = self.get_tp(old)?.1;
      if new.len() > MAX_TABLE_NAME { return Err(TableNameTooLong(new)); }
      tp.name_len = new.len() as u8;
      tp.name.as_mut_ptr().copy_from_nonoverlapping(new.as_ptr(), new.len());
      Ok(())
    }
  }
}

impl Db {
  pub fn drop_table<'a>(&mut self, table: &'a str) -> Result<'a, ()> {
    unsafe {
      let dp = self.dp();
      for (idx, &tp_id) in dp.tables().iter().enumerate() {
        let tp = self.get_page::<TablePage>(tp_id);
        if tp.name() == table {
          if self.foreign_links_to(tp_id).next().is_some() { return Err(ModifyTableWithForeignLink(table)); }
          let tables = dp.tables.as_mut_ptr();
          tables.add(idx).swap(tables.add(dp.table_num as usize - 1));
          dp.table_num -= 1;
          for ci in tp.cols() {
            if ci.index != !0 { self.dealloc_index(ci.index); }
            if ci.check != !0 { self.dealloc_page(ci.check >> 1); }
          }
          if tp.cols().iter().any(|ci| ci.ty.is_varchar()) {
            for (data, _) in self.record_iter(tp) {
              for (ci_id, ci) in tp.cols().iter().enumerate() {
                if !is_null(data, ci_id as u32) && ci.ty.is_varchar() {
                  self.free_varchar(data.add(ci.off as usize));
                }
              }
            }
          }
          self.drop_list(tp.first);
          return Ok(());
        }
      }
      Err(NoSuchTable(table))
    }
  }

  // `pub` for `index` crate's use
  pub unsafe fn drop_list(&mut self, mut first: u32) {
    while first != !0 {
      let next = self.get_page::<DataPage>(first).next;
      self.dealloc_page(first);
      first = next;
    }
  }
}