use std::{borrow::Cow, fs};

use common::{*, Error::*};
use syntax::ast::*;
use db::{Db, show::show_db};

#[derive(Default)]
pub struct Eval {
  pub db: Option<Db>
}

impl Eval {
  pub fn exec_all(&mut self, code: &str,
                  input_handler: impl Fn(&Stmt), result_handler: impl Fn(&str), err_handler: impl Fn(Error)) {
    match syntax::work(code) {
      Ok(program) => for s in &program {
        input_handler(s);
        match self.exec(s) { Ok(res) => result_handler(&res), Err(err) => err_handler(err), }
      }
      Err(err) => err_handler(err)
    }
  }

  pub fn exec_all_repl(&mut self, code: &str) {
    self.exec_all(code, |x| println!(">> {:?}", x), |x| println!("{}", x), |x| eprintln!("{:?}", x))
  }

  pub fn exec_all_check(&mut self, code: &str) {
    self.exec_all(code, |_| {}, |_| {}, |x| {
      eprintln!("{:?}", x);
      std::process::exit(1);
    })
  }

  pub fn exec<'a>(&mut self, sql: &Stmt<'a>) -> Result<'a, Cow<str>> {
    use Stmt::*;
    Ok(match sql {
      Insert(i) => (query::insert(i, self.db()?)?, "".into()).1,
      Delete(d) => (query::delete(d, self.db()?)?, "".into()).1,
      Select(s) => query::select(s, self.db()?)?.to_csv()?.into(),
      Update(u) => (query::update(u, self.db()?)?, "".into()).1,
      &CreateDb(path) => (Db::create(path), "".into()).1,
      &DropDb(path) => {
        if Some(path) == self.db.as_ref().map(|db| db.path()) { self.db = None; }
        (std::fs::remove_file(path)?, "".into()).1
      }
      &ShowDb(path) => {
        let mut s = String::new();
        (show_db(path, &mut s)?, s.into()).1
      }
      ShowDbs => {
        let mut s = String::new();
        for e in fs::read_dir(".")? {
          // `show_db` may fail because not all files are db format, just ignore these files
          let _ = show_db(e?.path(), &mut s);
        }
        s.into()
      }
      &UseDb(name) => (self.db = Some(Db::open(name)?), "".into()).1,
      CreateTable(c) => (self.db()?.create_table(c)?, "".into()).1,
      &DropTable(name) => (self.db()?.drop_table(name)?, "".into()).1,
      &ShowTable(name) => self.db()?.show_table(name)?.into(),
      ShowTables => self.db()?.show_tables().into(),
      &CreateIndex(table, col) => (index::create(self.db()?, table, col)?, "".into()).1,
      &DropIndex(table, col) => (self.db()?.drop_index(table, col)?, "".into()).1
    })
  }

  fn db<'a>(&mut self) -> Result<'a, &mut Db> { self.db.as_mut().ok_or(NoDbInUse) }
}