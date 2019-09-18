use common::*;

#[derive(Debug)]
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
  // (table, col)
  CreateIndex(&'a str, &'a str),
  DropIndex(&'a str, &'a str),
}

#[derive(Debug)]
pub struct Insert<'a> {
  pub table: &'a str,
  pub vals: Vec<Vec<Lit<'a>>>,
}

#[derive(Debug)]
pub struct Update<'a> {
  pub table: &'a str,
  pub sets: Vec<(&'a str, Lit<'a>)>,
  pub where_: Vec<Expr<'a>>,
}

#[derive(Debug)]
pub struct Select<'a> {
  // None for select *
  pub ops: Option<Vec<Agg<'a>>>,
  pub tables: Vec<&'a str>,
  pub where_: Vec<Expr<'a>>,
}

#[derive(Debug)]
pub struct Delete<'a> {
  pub table: &'a str,
  pub where_: Vec<Expr<'a>>,
}

#[derive(Debug)]
pub struct ColRef<'a> {
  pub table: Option<&'a str>,
  pub col: &'a str,
}

// Agg for Aggregation
#[derive(Debug)]
pub struct Agg<'a> {
  pub col: ColRef<'a>,
  pub op: AggOp,
}

#[derive(Debug)]
pub enum AggOp { None, Avg, Sum, Min, Max }

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
  Check(Vec<Lit<'a>>),
}

#[derive(Debug)]
pub enum Expr<'a> {
  Cmp(CmpOp, ColRef<'a>, Atom<'a>),
  // true for `is null`, false for `is not null`
  Null(ColRef<'a>, bool),
  Like(ColRef<'a>, &'a str),
}

impl<'a> Expr<'a> {
  #[inline(always)]
  pub fn lhs_col(&self) -> &ColRef<'a> {
    match self { Expr::Cmp(_, l, _) | Expr::Null(l, _) | Expr::Like(l, _) => l }
  }

  #[inline(always)]
  pub fn rhs_col(&self) -> Option<&ColRef<'a>> {
    match self { Expr::Cmp(_, _, Atom::ColRef(r)) => Some(r), _ => None }
  }
}

#[derive(Debug, Copy, Clone)]
pub enum CmpOp { Lt, Le, Ge, Gt, Eq, Ne }

#[derive(Debug)]
pub enum Atom<'a> {
  ColRef(ColRef<'a>),
  Lit(Lit<'a>),
}