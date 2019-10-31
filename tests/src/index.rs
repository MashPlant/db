use driver::Eval;
use std::collections::BTreeSet;
use rand::prelude::*;
use syntax::ast::*;
use common::{*, BareTy::*};
use physics::*;
use index::Index;

fn lit<'a>(x: i32) -> CLit<'a> { CLit::new(Lit::Number(x as f64)) }

#[test]
fn index() {
  const N: usize = 10000;
  let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(19260817);
  let (mut ins, mut del, mut test) = (vec![0; N], vec![0; N], vec![0; N]);
  for &max in &[N / 100, N / 10, N, N * 10, N * 100] {
    let mut e = Eval::default();
    let mut map = BTreeSet::new();
    let (table, col); // init later
    macro_rules! ins {
      () => {
        e.exec(&Stmt::Insert(Insert { table: "index", vals: ins.iter().map(|x| vec![lit(*x)]).collect(), cols: None })).unwrap();
        for (idx, &ins) in ins.iter().enumerate() {
          map.insert((ins, idx as i32));
        }
      };
    }
    macro_rules! del {
      ($range: expr) => {
        for &d in &del[$range] {
          e.exec(&Stmt::Delete(Delete { table: "index", where_: vec![Cond::Cmp(CmpOp::Eq, ColRef { table: None, col: "id" }, Atom::Lit(lit(d)))] })).unwrap();
          let rm = map.range((&(d, 0))..(&(d, N as i32))).cloned().collect::<Vec<_>>();
          for x in rm { map.remove(&x); }
        }
      };
    }
    macro_rules! test {
      () => {
        unsafe { Index::<{Int}>::new(e.db().unwrap(), table, col).debug_check_all(); }
        for &t in &test {
          let index_count = e.select(&Select {
            ops: None,
            tables: vec!["index"],
            where_: vec![Cond::Cmp(CmpOp::Eq, ColRef { table: None, col: "id" }, Atom::Lit(lit(t)))],
          }).unwrap().row_count();
          let map_count = map.range((&(t, 0))..(&(t, N as i32))).count();
          assert_eq!(index_count, map_count);
        }
      };
    }
    for x in &mut ins { *x = rng.gen_range(0, max as i32); }
    (del.copy_from_slice(&ins), del.shuffle(&mut rng));
    (test.copy_from_slice(&ins), test.shuffle(&mut rng));
    e.exec(&Stmt::CreateDb("index")).unwrap();
    e.exec(&Stmt::UseDb("index")).unwrap();
    e.exec(&CreateTable { table: "index", cols: vec![ColDecl { col: "id", ty: ColTy::FixTy(FixTy { size: 0, ty: Int }), notnull: true, dft: None }], cons: vec![] }.into()).unwrap();
    e.exec(&CreateIndex { index: "id_index", table: "index", col: "id" }.into()).unwrap();
    unsafe { // modify IndexPage's cap to generate more splits
      let db = e.db().unwrap();
      let (tp_id, tp) = db.get_tp("index").unwrap();
      let ci = tp.get_ci("id").unwrap();
      table = tp_id;
      col = ci.idx(&tp.cols);
      db.get_page::<IndexPage>(ci.index).cap = 8;
    }
    ins!();
    test!();
    del!(..N / 2);
    test!();
    del!(N / 2..);
    test!();
    ins!();
    test!();
    e.exec(&Stmt::DropDb("index")).unwrap();
  }
}