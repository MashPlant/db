use driver::Eval;
use rand::prelude::*;
use syntax::ast::*;
use common::{*, BareTy::*};
use physics::*;

fn lit<'a>(x: usize) -> CLit<'a> { CLit::new(Lit::Number(x as f64)) }

#[test]
fn lob() {
  const N: usize = 20000;
  const MAX_LEN: usize = 200;
  const ALLOC_RATE: f64 = 0.8;
  let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(19260817);
  let mut e = Eval::default();
  e.exec(&Stmt::CreateDb("lob")).unwrap();
  e.exec(&Stmt::UseDb("lob")).unwrap();
  e.exec(&CreateTable {
    table: "lob",
    cols: vec![
      ColDecl { col: "id", ty: ColTy::FixTy(FixTy { size: 0, ty: Int }), notnull: true, dft: None },
      ColDecl { col: "v", ty: ColTy::Varchar((MAX_LEN * LOB_SLOT_SIZE) as u16), notnull: true, dft: None }
    ],
    cons: vec![],
  }.into()).unwrap();
  e.exec(&CreateIndex { index: "id_index", table: "lob", col: "id" }.into()).unwrap();
  let mut result = Vec::new();
  for i in 0..N {
    if rng.gen_bool(ALLOC_RATE) {
      let len = rng.gen_range(1, MAX_LEN + 1) * LOB_SLOT_SIZE;
      let mut vec = Vec::with_capacity(len);
      for _ in 0..len {
        vec.push(rng.gen_range(0, 0x80));
      }
      let str = String::from_utf8(vec).unwrap();
      e.exec(&Stmt::Insert(Insert { table: "lob", vals: vec![vec![lit(i), CLit::new(Lit::Str(&str))]], cols: None })).unwrap();
      result.push(Some(str));
    } else {
      if !result.is_empty() {
        let idx = rng.gen_range(0, result.len());
        e.exec(&Stmt::Delete(Delete { table: "lob", where_: vec![Cond::Cmp(CmpOp::Eq, ColRef { table: None, col: "id" }, Atom::Lit(lit(idx)))] })).unwrap();
        result[idx] = None;
      }
      result.push(None);
    }
  }
  for i in 0..N {
    let sel = e.select(&Select {
      ops: Some(vec![Agg { col: ColRef { table: None, col: "v" }, op: None }]),
      tables: vec!["lob"],
      where_: vec![Cond::Cmp(CmpOp::Eq, ColRef { table: None, col: "id" }, Atom::Lit(lit(i)))],
    }).unwrap();
    if let Some(str) = result[i].as_ref() {
      assert_eq!(sel.row_count(), 1);
      if let Lit::Str(str1) = sel.data[0].lit() { assert_eq!(str, str1); } else { panic!("not str"); }
    } else {
      assert_eq!(sel.row_count(), 0);
    }
  }
  e.exec(&Stmt::DropDb("lob")).unwrap();
}