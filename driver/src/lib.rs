use std::{borrow::Cow, fs};
use typed_arena::Arena;

use common::{*, Error::*};
use syntax::ast::*;
use db::{Db, show::show_db};
use query::SelectResult;

#[derive(Default)]
pub struct Eval(Option<Db>);

impl Eval {
  pub fn exec_all<'a>(&mut self, code: &'a str, alloc: &'a Arena<u8>, input_handler: impl Fn(&Stmt), result_handler: impl Fn(&str)) -> ModifyResult<'a, ()> {
    for s in &syntax::work(code, alloc)? {
      input_handler(s);
      result_handler(&self.exec(s)?);
    }
    Ok(())
  }

  pub fn exec<'a>(&mut self, sql: &Stmt<'a>) -> ModifyResult<'a, Cow<str>> {
    fn fmt<'a>(n: u32) -> Cow<'a, str> { Cow::Owned(format!("{} column(s) affected", n)) }
    use Stmt::*;
    Ok(match sql {
      Insert(i) => fmt(query::insert(i, self.db()?)?),
      Delete(d) => fmt(query::delete(d, self.db()?)?),
      Select(s) => query::select(s, self.db()?)?.csv().into(),
      Update(u) => fmt(query::update(u, self.db()?)?),
      &CreateDb(path) => (Db::create(path), "".into()).1,
      &DropDb(path) => {
        if Some(path) == self.0.as_ref().map(|db| db.path()) { self.0 = None; }
        (fs::remove_file(path)?, "".into()).1
      }
      &ShowDb(path) => {
        let mut s = String::new();
        (show_db(path, &mut s)?, s.into()).1
      }
      ShowDbs => {
        let mut s = String::new();
        for entry in fs::read_dir(".")? {
          // `show_db` may fail because not all files are db format, just ignore these files
          let _ = show_db(entry?.path(), &mut s);
        }
        s.into()
      }
      &UseDb(path) => (self.0 = Some(Db::open(path)?), "".into()).1,
      CreateTable(c) => (self.db()?.create_table(c)?, "".into()).1,
      &DropTable(table) => (index::drop_table(self.db()?, table)?, "".into()).1,
      &ShowTable(table) => self.db()?.show_table(table)?.into(),
      ShowTables => self.db()?.show_tables().into(),
      CreateIndex(c) => (index::create_index(self.db()?, c)?, "".into()).1,
      &DropIndex { index, table } => (self.db()?.drop_index(index, table)?, "".into()).1,
      &Rename { old, new } => (self.db()?.rename_table(old, new)?, "".into()).1,
      AddForeign(a) => (index::add_foreign(self.db()?, a)?, "".into()).1,
      &DropForeign { table, col } => (self.db()?.drop_foreign(table, col)?, "".into()).1,
      AddPrimary { table, cols } => (index::add_primary(self.db()?, table, cols)?, "".into()).1,
      DropPrimary { table, cols } => (index::drop_primary(self.db()?, table, cols)?, "".into()).1,
      AddCol { table, col } => (index::add_col(self.db()?, table, col)?, "".into()).1,
      &DropCol { table, col } => (index::drop_col(self.db()?, table, col)?, "".into()).1,
    })
  }

  pub fn select<'a, 'b>(&'b self, s: &Select<'a>) -> Result<'a, SelectResult<'b>> {
    query::select(s, self.0.as_ref().ok_or(NoDbInUse)?)
  }

  pub fn db<'a>(&mut self) -> Result<'a, &mut Db> { self.0.as_mut().ok_or(NoDbInUse) }
}