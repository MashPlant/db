#![feature(proc_macro_hygiene)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![allow(unused_unsafe)]

pub mod ast;
pub mod parser;

pub use crate::{ast::*, parser::*};

use typed_arena::Arena;

use common::{ParserError as PE, ParserErrorKind::*, Error};

pub fn work<'a>(code: &'a str, alloc: &'a Arena<u8>) -> Result<Vec<Stmt<'a>>, Error<'a>> {
  let mut p = Parser { pe: vec![], alloc };
  match p.parse(&mut Lexer::new(code.as_bytes())) {
    Ok(ss) if p.pe.is_empty() => Ok(ss),
    Err(t) => {
      match t.kind {
        TokenKind::_Err => p.pe.push(PE { line: t.line, col: t.col, kind: UnexpectedChar(t.piece[0] as char) }),
        _ => p.pe.push(PE { line: t.line, col: t.col, kind: SyntaxError }),
      }
      Err(Error::ParserErrors(p.pe.into()))
    }
    _ => Err(Error::ParserErrors(p.pe.into())),
  }
}