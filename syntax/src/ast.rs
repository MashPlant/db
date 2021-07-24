use common::*;
use std::fmt;

#[derive(derive_more::From, Debug)]
pub enum Stmt<'a> {
  #[from] Insert(Insert<'a>),
  #[from] Delete(Delete<'a>),
  #[from] Select(Select<'a>),
  #[from] Update(Update<'a>),
  CreateDb(&'a str),
  DropDb(&'a str),
  ShowDb(&'a str),
  ShowDbs,
  UseDb(&'a str),
  #[from] CreateTable(CreateTable<'a>),
  DropTable(&'a str),
  ShowTable(&'a str),
  ShowTables,
  #[from] CreateIndex(CreateIndex<'a>),
  DropIndex {
    index: &'a str,
    // `table` is only for check, doesn't provide any information
    // "drop index" => table is None; "alter table drop index" => table is Some
    table: Option<&'a str>,
  },
  Rename { old: &'a str, new: &'a str },
  #[from] AddForeign(AddForeign<'a>),
  DropForeign { table: &'a str, col: &'a str },
  AddPrimary { table: &'a str, cols: Vec<&'a str> },
  DropPrimary { table: &'a str, cols: Vec<&'a str> },
  AddCol { table: &'a str, col: ColDecl<'a> },
  DropCol { table: &'a str, col: &'a str },
}

#[derive(Debug)]
pub struct Insert<'a> {
  pub table: &'a str,
  pub cols: Option<Vec<&'a str>>,
  pub vals: Vec<Vec<CLit<'a>>>,
}

#[derive(Debug)]
pub struct Update<'a> {
  pub table: &'a str,
  pub sets: Vec<(&'a str, Expr<'a>)>,
  pub where_: Vec<Cond<'a>>,
}

#[derive(Debug)]
pub struct Select<'a> {
  // None for select *
  pub ops: Option<Vec<Agg<'a>>>,
  pub tables: Vec<&'a str>,
  pub where_: Vec<Cond<'a>>,
}

#[derive(Debug)]
pub struct Delete<'a> {
  pub table: &'a str,
  pub where_: Vec<Cond<'a>>,
}

#[derive(Copy, Clone)]
pub struct ColRef<'a> {
  pub table: Option<&'a str>,
  pub col: &'a str,
}

// Agg is short for Aggregation
pub struct Agg<'a> {
  pub col: ColRef<'a>,
  pub op: Option<AggOp>,
}

#[derive(Debug)]
pub struct CreateTable<'a> {
  pub table: &'a str,
  pub cols: Vec<ColDecl<'a>>,
  pub cons: Vec<ColCons<'a>>,
}

#[derive(Debug)]
pub struct CreateIndex<'a> {
  pub index: &'a str,
  pub table: &'a str,
  pub col: &'a str,
}

#[derive(Debug)]
pub struct AddForeign<'a> {
  pub table: &'a str,
  pub col: &'a str,
  pub f_table: &'a str,
  pub f_col: &'a str,
}

#[derive(Debug)]
pub struct ColDecl<'a> {
  pub col: &'a str,
  pub ty: ColTy,
  pub notnull: bool,
  pub dft: Option<CLit<'a>>,
}

// Cons for Constraint
#[derive(Debug)]
pub enum ColCons<'a> {
  Primary(Vec<&'a str>),
  Foreign { col: &'a str, f_table: &'a str, f_col: &'a str },
  Unique(&'a str),
  Check(&'a str, Vec<CLit<'a>>),
}

#[derive(Copy, Clone)]
pub enum Cond<'a> {
  Cmp(CmpOp, ColRef<'a>, Atom<'a>),
  // true for `is null`, false for `is not null`
  Null(ColRef<'a>, bool),
  Like(ColRef<'a>, &'a str),
}

// this is arithmetic expr, only appears in the set list of update, not in where list of select and delete
// Cond is a proper subset of Expr
pub enum Expr<'a> {
  Atom(Atom<'a>),
  Null(Box<Expr<'a>>, bool),
  Like(Box<Expr<'a>>, &'a str),
  And(Box<(Expr<'a>, Expr<'a>)>),
  Or(Box<(Expr<'a>, Expr<'a>)>),
  Cmp(CmpOp, Box<(Expr<'a>, Expr<'a>)>),
  Bin(BinOp, Box<(Expr<'a>, Expr<'a>)>),
}

impl<'a> Cond<'a> {
  pub fn lhs_col(&self) -> &ColRef<'a> {
    match self { Cond::Cmp(_, l, _) | Cond::Null(l, _) | Cond::Like(l, _) => l }
  }

  pub fn rhs_col_op(&self) -> Option<(&ColRef<'a>, CmpOp)> {
    match self { Cond::Cmp(op, _, Atom::ColRef(r)) => Some((r, *op)), _ => None }
  }
}

#[derive(Copy, Clone)]
pub enum Atom<'a> { ColRef(ColRef<'a>), Lit(CLit<'a>) }

impl fmt::Debug for ColRef<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(table) = self.table { write!(f, "{}.{}", table, self.col) } else { write!(f, "{}", self.col) }
  }
}

impl fmt::Debug for Agg<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(op) = self.op { write!(f, "{}({:?})", op.name(), self.col) } else { write!(f, "{:?}", self.col) }
  }
}

impl fmt::Debug for Atom<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self { Atom::ColRef(c) => write!(f, "{:?}", c), Atom::Lit(l) => write!(f, "{:?}", l) }
  }
}

impl fmt::Debug for Cond<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Cond::Cmp(op, l, r) => write!(f, "{:?} {} {:?}", l, op.name(), r),
      Cond::Null(x, null) => write!(f, "{:?} is {}null", x, if *null { "" } else { "not " }),
      Cond::Like(x, like) => write!(f, "{:?} like '{}'", x, like),
    }
  }
}

impl fmt::Debug for Expr<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Expr::Atom(x) => write!(f, "{:?}", x),
      Expr::Null(x, null) => write!(f, "({:?}) is {}null", x, if *null { "" } else { "not " }),
      Expr::Like(x, like) => write!(f, "({:?}) like '{}'", x, like),
      Expr::And(box (l, r)) => write!(f, "({:?}) and ({:?})", l, r), Expr::Or(box (l, r)) => write!(f, "({:?}) or ({:?})", l, r),
      Expr::Cmp(op, box (l, r)) => write!(f, "({:?}) {} ({:?})", l, op.name(), r), Expr::Bin(op, box (l, r)) => write!(f, "({:?}) {} ({:?})", l, op.name(), r),
    }
  }
}