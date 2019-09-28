#![allow(unused)]
mod integrate;

use driver::Eval;

pub(crate) fn exec_all(code: &[u8], e: Option<Eval>) -> Eval {
  use syntax::*;
  let mut e = e.unwrap_or_else(Eval::default);
  for s in &Parser.parse(&mut Lexer::new(code)).unwrap() {
    let res = e.exec(s);
    res.unwrap();
  }
  e
}