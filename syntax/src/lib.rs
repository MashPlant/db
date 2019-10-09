#![feature(proc_macro_hygiene)]
#![feature(box_patterns)]
pub mod ast;
pub mod parser;

pub use crate::{ast::*, parser::*};

use common::{ParserError as PE, ParserErrorKind::*, Error};

pub fn work(code: &str) -> std::result::Result<Vec<Stmt>, Error> {
  let mut p = Parser(vec![]);
  match p.parse(&mut Lexer::new(code.as_bytes())) {
    Ok(ss) if p.0.is_empty() => Ok(ss),
    Err(t) => {
      match t.ty {
        TokenKind::_Err => p.0.push(PE { line: t.line, col: t.col, kind: UnrecognizedChar(t.piece[0] as char) }),
        _ => p.0.push(PE { line: t.line, col: t.col, kind: SyntaxError }),
      }
      Err(Error::ParserErrors(p.0.into()))
    }
    _ => Err(Error::ParserErrors(p.0.into())),
  }
}