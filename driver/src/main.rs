use driver::Eval;

fn exec_all(code: &[u8], e: &mut Eval) {
  use syntax::*;

  for s in &Parser.parse(&mut Lexer::new(code)).unwrap() {
//    println!(">> {:?}", s);
    match e.exec(s) {
      Ok(msg) => print!("{}", msg),
      Err(err) => println!("Error: {}", err),
    }
  }
}

fn main() {
  let ref mut e = Eval::default();

//  exec_all(include_bytes!("../../tests/sql/create.sql"), e);
//  exec_all(include_bytes!("../../tests/sql/customer.sql"), e);
  exec_all(include_bytes!("../../tests/sql/test_select.sql"), e);


//  let file: &[u8] = include_bytes!("../../tests/sql/customer.sql");
//  let sl = Parser.parse(&mut Lexer::new(file)).unwrap();
//  for s in &sl {
//    println!(">> {:?}", s);
//    match e.exec(s) {
//      Ok(msg) => print!("{}", msg),
//      Err(err) => println!("Error: {}", err),
//    }
//  }
}

//use serde::{Serialize, Deserialize};
//
//#[derive(Serialize, Deserialize)]
//struct A<'a> {
//  s: Option<&'a str>,
//}
//  let s = "123".to_owned();
//  let a = A { s: Some(&s) };
//  let bc = bincode::serialize(&a).unwrap();
//  println!("{}", bc.len());
//  let a = bincode::deserialize::<A>(&bc).unwrap();
//  println!("{}", a.s.unwrap());