use driver::Eval;
use std::collections::BTreeSet;
use rand::prelude::*;
use syntax::ast::*;
use common::{*, BareTy::*};
use physics::*;
use index::Index;

#[test]
#[ignore]
pub(crate) fn test_index() {
  const N: usize = 10000;
  let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(10);
  let (mut ins, mut del, mut test) = (vec![0; N], vec![0; N], vec![0; N]);
  for &max in &[N / 100, N / 10, N, N * 10, N * 100] {
    let mut e = Eval::default();
    let mut map = BTreeSet::new();
    let mut rid = Rid::new(0, 0); // init later
    macro_rules! test {
      () => {
        unsafe { Index::<{Int}>::new(e.db.as_mut().unwrap(), rid).debug_check_all(); }
        for &t in &test {
          let index_count = e.exec(&Stmt::Select(Select {
            ops: None,
            tables: vec!["test"],
            where_: vec![Expr::Cmp(CmpOp::Eq, ColRef { table: None, col: "id" }, Atom::Lit(Lit::Int(t)))],
          })).unwrap().lines().count() - 1; // csv, ignore first line
          let map_count = map.range((&(t, 0))..(&(t, N as i32))).count();
          assert_eq!(index_count, map_count);
        }
      };
    }
    for x in &mut ins {
      *x = rng.gen_range(0, max as i32);
    }
    (del.copy_from_slice(&ins), del.shuffle(&mut rng));
    (test.copy_from_slice(&ins), test.shuffle(&mut rng));
    e.exec(&Stmt::CreateDb("test")).unwrap();
    e.exec(&Stmt::UseDb("test")).unwrap();
    e.exec(&Stmt::CreateTable(CreateTable { name: "test", cols: vec![ColDecl { name: "id", ty: ColTy { size: 0, ty: Int }, notnull: true }], cons: vec![] })).unwrap();
    e.exec(&Stmt::CreateIndex { table: "test", col: "id" }).unwrap();
    unsafe { // modify IndexPage's cap to generate more splits
      let db = e.db.as_mut().unwrap();
      let ti = db.dp().get_ti("test").unwrap();
      let tp = db.get_page::<TablePage>(ti.meta);
      let ci = tp.get_ci("id").unwrap();
      rid = Rid::new(ti.meta, ci.idx(&tp.cols));
      db.get_page::<IndexPage>(ci.index).cap = 8;
    }
    e.exec(&Stmt::Insert(Insert { table: "test", vals: ins.iter().map(|x| vec![Lit::Int(*x)]).collect() })).unwrap();
    for (idx, &ins) in ins.iter().enumerate() {
      map.insert((ins, idx as i32));
    }
    test!();
    for (_idx, &d) in del[0..N / 2].iter().enumerate() {
      e.exec(&Stmt::Delete(Delete { table: "test", where_: vec![Expr::Cmp(CmpOp::Eq, ColRef { table: None, col: "id" }, Atom::Lit(Lit::Int(d)))] })).unwrap();
      let rm = map.range((&(d, 0))..(&(d, N as i32))).cloned().collect::<Vec<_>>();
      for x in rm {
        map.remove(&x);
      }
    }
    test!();
    e.exec(&Stmt::DropDb("test")).unwrap();
  }
}