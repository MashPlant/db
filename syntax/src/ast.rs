use common::*;
use std::fmt;

pub enum Stmt<'a> {
  Insert(Insert<'a>),
  Delete(Delete<'a>),
  Select(Select<'a>),
  Update(Update<'a>),
  CreateDb(&'a str),
  DropDb(&'a str),
  ShowDb(&'a str),
  ShowDbs,
  UseDb(&'a str),
  CreateTable(CreateTable<'a>),
  DropTable(&'a str),
  ShowTable(&'a str),
  ShowTables,
  CreateIndex { table: &'a str, col: &'a str },
  DropIndex { table: &'a str, col: &'a str },
}

#[derive(Debug)]
pub struct Insert<'a> {
  pub table: &'a str,
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
  pub name: &'a str,
  pub cols: Vec<ColDecl<'a>>,
  pub cons: Vec<TableCons<'a>>,
}

#[derive(Debug)]
pub struct ColDecl<'a> {
  pub name: &'a str,
  pub ty: ColTy,
  pub notnull: bool,
}

// Cons for Constraint
#[derive(Debug)]
pub struct TableCons<'a> {
  pub name: &'a str,
  pub kind: TableConsKind<'a>,
}

#[derive(Debug)]
pub enum TableConsKind<'a> {
  Primary,
  Foreign { table: &'a str, col: &'a str },
  Unique,
  Check(Vec<CLit<'a>>),
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
  Neg(Box<Expr<'a>>),
  And(Box<(Expr<'a>, Expr<'a>)>),
  Or(Box<(Expr<'a>, Expr<'a>)>),
  Cmp(CmpOp, Box<(Expr<'a>, Expr<'a>)>),
  Bin(BinOp, Box<(Expr<'a>, Expr<'a>)>),
}

impl<'a> Cond<'a> {
  pub fn lhs_col(&self) -> &ColRef<'a> {
    match self { Cond::Cmp(_, l, _) | Cond::Null(l, _) | Cond::Like(l, _) => l }
  }

  pub fn rhs_col(&self) -> Option<&ColRef<'a>> {
    match self { Cond::Cmp(_, _, Atom::ColRef(r)) => Some(r), _ => None }
  }
}

#[derive(Copy, Clone)]
pub enum Atom<'a> {
  ColRef(ColRef<'a>),
  Lit(CLit<'a>),
}

impl fmt::Debug for Stmt<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use Stmt::*;
    match self {
      Insert(x) => write!(f, "{:?}", x), Delete(x) => write!(f, "{:?}", x), Select(x) => write!(f, "{:?}", x), Update(x) => write!(f, "{:?}", x), CreateTable(x) => write!(f, "{:?}", x),
      CreateDb(x) => write!(f, "CreateDb({:?})", x), DropDb(x) => write!(f, "DropDb({:?})", x), ShowDb(x) => write!(f, "ShowDb({:?})", x), UseDb(x) => write!(f, "UseDb({:?})", x), DropTable(x) => write!(f, "DropTable({:?})", x), ShowTable(x) => write!(f, "ShowTable({:?})", x),
      ShowDbs => write!(f, "ShowDbs"), ShowTables => write!(f, "ShowTables"),
      CreateIndex { table, col } => write!(f, "CreateIndex({}.{})", table, col), DropIndex { table, col } => write!(f, "DropIndex({}.{})", table, col)
    }
  }
}

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
      Expr::Neg(x) => write!(f, "-({:?})", x),
      Expr::And(box (l, r)) => write!(f, "({:?}) and ({:?})", l, r), Expr::Or(box (l, r)) => write!(f, "({:?}) or ({:?})", l, r),
      Expr::Cmp(op, box (l, r)) => write!(f, "({:?}) {} ({:?})", l, op.name(), r), Expr::Bin(op, box (l, r)) => write!(f, "({:?}) {} ({:?})", l, op.name(), r),
    }
  }
}