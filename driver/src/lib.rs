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

  pub fn exec(&mut self, sql: &Stmt) -> Result<Cow<str>> {
    use Stmt::*;
    match sql {
      Insert(i) => { query::insert(i, self.db()?).map(|_| "".into()) }
      Delete(d) => { query::delete(d, self.db()?).map(|_| "".into()) }
      Select(s) => { query::select(s, self.db()?).map(|s| s.to_string().into()) }
      Update(u) => { query::update(u, self.db()?).map(|_| "".into()) }
      &CreateDb(path) => Db::create(path).map(|_| "".into()),
      &DropDb(path) => {
        if Some(path) == self.db.as_ref().map(|db| db.path()) { self.db = None; }
        std::fs::remove_file(path)?;
        Ok("".into())
      }
      &ShowDb(path) => {
        let mut s = String::new();
        show_db(path, &mut s)?;
        Ok(s.into())
      }
      ShowDbs => {
        let mut s = String::new();
        for e in fs::read_dir(".")? {
          let _ = show_db(e?.path(), &mut s);
        }
        Ok(s.into())
      }
      &UseDb(name) => {
        self.db = Some(Db::open(name)?);
        Ok("".into())
      }
      CreateTable(c) => self.db()?.create_table(c).map(|_| "".into()),
      &DropTable(name) => self.db()?.drop_table(name).map(|_| "".into()),
      &ShowTable(name) => self.db()?.show_table(name).map(|s| s.into()),
      ShowTables => Ok(self.db()?.show_tables().into()),
      &CreateIndex(table, col) => {
        index::create(self.db()?, table, col)?;
        Ok("".into())
      }
      &DropIndex(table, col) => {
        self.db()?.drop_index(table, col)?;
        Ok("".into())
      }
    }
  }

  fn db(&mut self) -> Result<&mut Db> { self.db.as_mut().ok_or(NoDbInUse) }
}