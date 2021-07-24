use rustyline::{Editor, Helper, highlight::Highlighter, completion::Completer, hint::Hinter, validate::Validator, error::ReadlineError};
use colored::*;
use std::{borrow::Cow, str, fs};
use typed_arena::Arena;

use driver::Eval;
use syntax::{Lexer, TokenKind};

struct SqlHelper;

impl Highlighter for SqlHelper {
  #[cfg_attr(tarpaulin, ignore)]
  fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
    use TokenKind::*;
    let mut lexer = Lexer::new(line.as_bytes());
    let mut ret = line.to_owned();
    loop {
      let token = lexer.next();
      let piece = str::from_utf8(token.piece).unwrap();
      let start = token.col as usize - 1 + ret.len() - line.len();
      let range = start..start + piece.len();
      match token.kind {
        Lt | Le | Ge | Gt | Eq | Ne | LPar | RPar | Add | Sub | Mul | Div | Mod | Comma | Semicolon => {}
        Null | True | False | FloatLit | IntLit | StrLit => ret.replace_range(range, &piece.green().to_string()),
        Int | Bool | Char | Varchar | Float | Date => ret.replace_range(range, &piece.cyan().to_string()),
        Sum | Avg | Min | Max | Count => ret.replace_range(range, &piece.yellow().to_string()),
        Id1 | Dot => ret.replace_range(range, &piece.purple().to_string()),
        _Err | _Eof => break ret.into(),
        _ => ret.replace_range(range, &piece.blue().bold().to_string()),
      }
    }
  }

  fn highlight_char(&self, _line: &str, _pos: usize) -> bool { true }
}

impl Completer for SqlHelper { type Candidate = String; }
impl Hinter for SqlHelper { type Hint = String; }
impl Helper for SqlHelper {}
impl Validator for SqlHelper {}

fn main() {
  let mut rl = Editor::new();
  rl.set_helper(Some(SqlHelper));
  let mut cur = String::new();
  let mut e = Eval::default();
  let mut output = None;
  println!("Database repl by MashPlant. Enter sql statement separated by semicolon.");
  loop {
    match rl.readline(if cur.is_empty() { ">> " } else { ".. " }) {
      Ok(line) => {
        let line = line.trim();
        if line.is_empty() { continue; }
        rl.add_history_entry(line);
        if cur.is_empty() && line.starts_with('.') {
          let mut words = line.split_whitespace();
          let cmd = words.next().unwrap();
          const OUTPUT: &str = ".output";
          const READ: &str = ".read";
          const COLOR: &str = ".color";
          match cmd {
            OUTPUT => output = words.next().map(|x| x.to_owned()),
            READ => if let Some(file) = words.next() {
              if let Ok(input) = fs::read_to_string(file) {
                if let Err(e) = e.exec_all(&input, &Arena::default(), |_| {}, |_| {}) { eprintln!("Error: {:?}", e); }
              } else { eprintln!("Error: fails to read from {}", file); }
            } else { eprintln!("Usage: {} <file>", READ); }
            COLOR => if let Some(color) = words.next().and_then(|x| x.parse().ok()) {
              rl.set_helper(if color { Some(SqlHelper) } else { None });
            } else { eprintln!("Usage: {} [true|false]", COLOR); }
            _ => eprintln!("Unknown command: {}", cmd),
          }
        } else {
          cur += line;
          cur.push('\n');
          if line.contains(';') {
            if let Err(e) = e.exec_all(&cur, &Arena::default(), |_| {}, |x| if !x.is_empty() {
              if let Some(output) = &output {
                if fs::write(output, x).is_err() { eprintln!("Error: fails to write to {}", output); }
              } else { println!("{}", x); }
            }) { eprintln!("Error: {:?}", e); }
            cur.clear();
          }
        }
      }
      Err(ReadlineError::Interrupted) => {}
      _ => break,
    }
  }
}