fn main() {
  use syntax::*;
  use driver::Eval;

  let file: &[u8] = include_bytes!("../../tests/sql/create.sql");
  let sl = Parser.parse(&mut Lexer::new(file)).unwrap();
  let mut e = Eval::new();
  for s in &sl {
    println!(">> {:?}", s);
    match e.exec(s) {
      Ok(msg) => print!("{}", msg),
      Err(err) => println!("Error: {}", err),
    }
  }

  let file: &[u8] = include_bytes!("../../tests/sql/customer.sql");
  let sl = Parser.parse(&mut Lexer::new(file)).unwrap();
  for s in &sl {
    println!(">> {:?}", s);
    match e.exec(s) {
      Ok(msg) => print!("{}", msg),
      Err(err) => println!("Error: {}", err),
    }
  }
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