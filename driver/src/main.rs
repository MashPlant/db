use typed_arena::Arena;

use driver::Eval;

fn exec_repl(e: &mut Eval, code: &str) {
  match &syntax::work(code, &Arena::default()) {
    Ok(ss) => for s in ss {
      println!(">> {:?}", s);
      match e.exec(s) { Ok(res) => if !res.is_empty() { println!("{}", res); }, Err(e) => eprintln!("Error: {:?}", e) }
    }
    Err(e) => eprintln!("Error: {:?}", e),
  }
}

fn main() {
  let mut e = Eval::default();
  exec_repl(&mut e, include_str!("../../tests/sql/test_select.sql"));
}