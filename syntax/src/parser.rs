use std::str::{self, FromStr};
use typed_arena::Arena;

use common::{BareTy::{*, self}, FixTy, ColTy, ParserError as PE, ParserErrorKind::*, Lit, CLit, AggOp::*, BinOp::*, CmpOp::*};
use crate::ast::*;
use crate::Stmt::AddPrimary;

pub struct Parser<'a> {
  pub pe: Vec<PE<'a>>,
  // allocator for string
  pub alloc: &'a Arena<u8>,
}

impl<'p> Parser<'p> {
  // it seems that sql doesn't support any escape characters (like \n, \t), in order to represent ', it uses ''
  fn escape(&self, s: &'p str) -> &'p str {
    if s.contains("''") {
      let s = s.replace("''", "'");
      let s = self.alloc.alloc_extend(s.bytes());
      unsafe { str::from_utf8_unchecked(s) }
    } else { s }
  }
}

impl<'p> Token<'p> {
  fn str_trim(&self) -> &'p str { unsafe { str::from_utf8_unchecked(self.piece.get_unchecked(1..self.piece.len() - 1)) } }
  fn str(&self) -> &'p str { unsafe { str::from_utf8_unchecked(self.piece) } }
  fn parse<T: FromStr + Default, U>(&self, ok: impl Fn(T) -> U, mut err: impl FnMut(u32, u32, &'p str)) -> U {
    let s = self.str();
    ok(s.parse().unwrap_or_else(|_| (err(self.line, self.col, s), T::default()).1))
  }
}

type FieldList<'p> = (Vec<ColDecl<'p>>, Vec<ColCons<'p>>);

#[parser_macros::lalr1(Program)]
#[use_unsafe]
#[lex(r##"
priority = [
  { assoc = 'left', terms = ['Or'] },
  { assoc = 'left', terms = ['And'] },
  { assoc = 'no_assoc', terms = ['Eq', 'Ne'] },
  { assoc = 'no_assoc', terms = ['Le', 'Ge', 'Lt', 'Gt'] },
  { assoc = 'left', terms = ['Add', 'Sub'] },
  { assoc = 'left', terms = ['Mul', 'Div', 'Mod'] },
  { assoc = 'no_assoc', terms = ['Is', 'Like'] },
  { assoc = 'no_assoc', terms = ['UMinus'] },
  { assoc = 'no_assoc', terms = ['RPar'] },
]

[lexical] # I hate sql...
'(c|C)(r|R)(e|E)(a|A)(t|T)(e|E)' = 'Create'
'(d|D)(r|R)(o|O)(p|P)' = 'Drop'
'(u|U)(s|S)(e|E)' = 'Use'
'(s|S)(h|H)(o|O)(w|W)' = 'Show'
'(d|D)(e|E)(s|S)(c|C)' = 'Desc'
'(a|A)(l|L)(t|T)(e|E)(r|R)\s+(t|T)(a|A)(b|B)(l|L)(e|E)' = 'AlterTable'
'(a|A)(d|D)(d|D)' = 'Add1'
'(r|R)(e|E)(n|N)(a|A)(m|M)(e|E)\s+(t|T)(o|O)' = 'RenameTo'
'(d|D)(a|A)(t|T)(a|A)(b|B)(a|A)(s|S)(e|E)(s|S)' = 'DataBases'
'(d|D)(a|A)(t|T)(a|A)(b|B)(a|A)(s|S)(e|E)' = 'DataBase'
'(t|T)(a|A)(b|B)(l|L)(e|E)(s|S)' = 'Tables'
'(t|T)(a|A)(b|B)(l|L)(e|E)' = 'Table'
'(s|S)(e|E)(l|L)(e|E)(c|C)(t|T)' = 'Select'
'(d|D)(e|E)(l|L)(e|E)(t|T)(e|E)' = 'Delete'
'(i|I)(n|N)(s|S)(e|E)(r|R)(t|T)\s+(i|I)(n|N)(t|T)(o|O)' = 'InsertInto'
'(u|U)(p|P)(d|D)(a|A)(t|T)(e|E)' = 'Update'
'(v|V)(a|A)(l|L)(u|U)(e|E)(s|S)' = 'Values'
'(r|R)(e|E)(f|F)(e|E)(r|R)(e|E)(n|N)(c|C)(e|E)(s|S)' = 'References'
'(s|S)(e|E)(t|T)' = 'Set'
'(f|F)(r|R)(o|O)(m|M)' = 'From'
'(w|W)(h|H)(e|E)(r|R)(e|E)' = 'Where'
'(s|S)(u|U)(m|M)' = 'Sum'
'(a|A)(v|V)(g|G)' = 'Avg'
'(m|M)(i|I)(n|N)' = 'Min'
'(m|M)(a|A)(x|X)' = 'Max'
'(c|C)(o|O)(u|U)(n|N)(t|T)' = 'Count'
'(n|N)(o|O)(t|T)\s+(n|N)(u|U)(l|L)(l|L)' = 'NotNull'
'(p|P)(r|R)(i|I)(m|M)(a|A)(r|R)(y|Y)\s+(k|K)(e|E)(y|Y)' = 'PrimaryKey'
'(f|F)(o|O)(r|R)(e|E)(i|I)(g|G)(n|N)\s+(k|K)(e|E)(y|Y)' = 'ForeignKey'
'(u|U)(n|N)(i|I)(q|Q)(u|U)(e|E)' = 'Unique'
'(l|L)(i|I)(k|K)(e|E)' = 'Like'
'(i|I)(n|N)(d|D)(e|E)(x|X)' = 'Index'
'(c|C)(h|H)(e|E)(c|C)(k|K)' = 'Check'
'(d|D)(e|E)(f|F)(a|A)(u|U)(l|L)(t|T)' = 'Default'
'(i|I)(n|N)' = 'In'
'(o|O)(n|N)' = 'On'
'(i|I)(s|S)' = 'Is'
'(b|B)(i|I)(g|G)(i|I)(n|N)(t|T)' = 'Int' # handle bigint as int, decimal as float
'(i|I)(n|N)(t|T)(e|E)(g|G)(e|E)(r|R)' = 'Int'
'(i|I)(n|N)(t|T)' = 'Int'
'(b|B)(o|O)(o|O)(l|L)' = 'Bool'
'(c|C)(h|H)(a|A)(r|R)' = 'Char'
'(v|V)(a|A)(r|R)(c|C)(h|H)(a|A)(r|R)' = 'Varchar'
'(d|D)(e|E)(c|C)(i|I)(m|M)(a|A)(l|L)' = 'Float'
'(f|F)(l|L)(o|O)(a|A)(t|T)' = 'Float'
'(d|D)(a|A)(t|T)(e|E)' = 'Date'
'(a|A)(n|N)(d|D)' = 'And'
'(o|O)(r|R)' = 'Or'
'(n|N)(u|U)(l|L)(l|L)' = 'Null'
'(t|T)(r|R)(u|U)(e|E)' = 'True'
'(f|F)(a|A)(l|L)(s|S)(e|E)' = 'False'
'<' = 'Lt'
'<=' = 'Le'
'>=' = 'Ge'
'>' = 'Gt'
'=' = 'Eq'
'(<>)|(!=)' = 'Ne'
'\(' = 'LPar'
'\)' = 'RPar'
'\+' = 'Add'
'-' = 'Sub'
'\*' = 'Mul'
'/' = 'Div'
'%' = 'Mod'
'\.' = 'Dot'
',' = 'Comma'
';' = 'Semicolon'
'--[^\n]*' = '_Eps'
'\s+' = '_Eps'
'-?\d+\.\d*' = 'FloatLit'
'-?\d+' = 'IntLit'
"'(('')|[^'])*'" = 'StrLit'
'[A-Za-z]\w*' = 'Id1'
'.' = '_Err'
"##)]
impl<'p> Parser<'p> {
  #[rule(Id -> Id1)]
  fn id(t: Token) -> &'p str { t.str() }

  #[rule(Program ->)]
  fn stmt_list0() -> Vec<Stmt<'p>> { vec![] }
  #[rule(Program -> Program Stmt Semicolon)]
  fn stmt_list1(mut sl: Vec<Stmt<'p>>, s: Stmt<'p>, _: Token) -> Vec<Stmt<'p>> { (sl.push(s), sl).1 }

  #[rule(Stmt -> Show DataBases)]
  fn stmt_show_dbs(_: Token, _: Token) -> Stmt<'p> { Stmt::ShowDbs }
  #[rule(Stmt -> Show DataBase Id)]
  fn stmt_show_db(_: Token, _: Token, db: &'p str) -> Stmt<'p> { Stmt::ShowDb(db) }
  #[rule(Stmt -> Create DataBase Id)]
  fn stmt_create_db(_: Token, _: Token, db: &'p str) -> Stmt<'p> { Stmt::CreateDb(db) }
  #[rule(Stmt -> Drop DataBase Id)]
  fn stmt_drop_db(_: Token, _: Token, db: &'p str) -> Stmt<'p> { Stmt::DropDb(db) }
  #[rule(Stmt -> Use Id)]
  fn stmt_use_db0(_: Token, db: &'p str) -> Stmt<'p> { Stmt::UseDb(db) }
  #[rule(Stmt -> Use DataBase Id)]
  fn stmt_use_db1(_: Token, _: Token, db: &'p str) -> Stmt<'p> { Stmt::UseDb(db) }
  #[rule(Stmt -> Drop Table Id)]
  fn stmt_drop_table(_: Token, _: Token, table: &'p str) -> Stmt<'p> { Stmt::DropTable(table) }
  #[rule(Stmt -> Create Index Id On Id LPar Id RPar)]
  fn stmt_create_index(_: Token, _: Token, index: &'p str, _: Token, table: &'p str, _: Token, col: &'p str, _: Token) -> Stmt<'p> { CreateIndex { index, table, col }.into() }
  #[rule(Stmt -> Drop Index Id)]
  fn stmt_drop_index(_: Token, _: Token, index: &'p str) -> Stmt<'p> { Stmt::DropIndex { index, table: None } }
  #[rule(Stmt -> Create Table Id LPar FieldList RPar)]
  fn stmt_create_table(_: Token, _: Token, table: &'p str, _: Token, (cols, cons): FieldList<'p>, _: Token) -> Stmt<'p> { CreateTable { table, cols, cons }.into() }
  #[rule(Stmt -> Show Tables)]
  fn stmt_show_tables(_: Token, _: Token) -> Stmt<'p> { Stmt::ShowTables }
  #[rule(Stmt -> Desc Id)]
  fn stmt_show_table0(_: Token, table: &'p str) -> Stmt<'p> { Stmt::ShowTable(table) }
  #[rule(Stmt -> Show Table Id)]
  fn stmt_show_table1(_: Token, _: Token, table: &'p str) -> Stmt<'p> { Stmt::ShowTable(table) }
  #[rule(Stmt -> Select Mul From IdList WhereM)]
  fn stmt_select0(_: Token, _: Token, _: Token, tables: Vec<&'p str>, where_: Vec<Cond<'p>>) -> Stmt<'p> { Select { ops: None, tables, where_ }.into() }
  #[rule(Stmt -> Select AggList From IdList WhereM)]
  fn stmt_select1(_: Token, ops: Vec<Agg<'p>>, _: Token, tables: Vec<&'p str>, where_: Vec<Cond<'p>>) -> Stmt<'p> { Select { ops: Some(ops), tables, where_ }.into() }
  #[rule(Stmt -> InsertInto Id Values LitListList)]
  fn stmt_insert0(_: Token, table: &'p str, _: Token, vals: Vec<Vec<CLit<'p>>>) -> Stmt<'p> { Insert { table, cols: None, vals }.into() }
  #[rule(Stmt -> InsertInto Id LPar IdList RPar Values LitListList)]
  fn stmt_insert1(_: Token, table: &'p str, _: Token, cols: Vec<&'p str>, _: Token, _: Token, vals: Vec<Vec<CLit<'p>>>) -> Stmt<'p> { Insert { table, cols: Some(cols), vals }.into() }
  #[rule(Stmt -> Update Id Set SetList WhereM)]
  fn stmt_update(_: Token, table: &'p str, _: Token, sets: Vec<(&'p str, Expr<'p>)>, where_: Vec<Cond<'p>>) -> Stmt<'p> { Update { table, sets, where_ }.into() }
  #[rule(Stmt -> Delete From Id WhereM)]
  fn stmt_delete(_: Token, _: Token, table: &'p str, where_: Vec<Cond<'p>>) -> Stmt<'p> { Delete { table, where_ }.into() }

  #[rule(Stmt -> AlterTable Id Add1 Index Id On LPar Id RPar)]
  fn alter_create_index1(_: Token, table: &'p str, _: Token, _: Token, index: &'p str, _: Token, _: Token, col: &'p str, _: Token) -> Stmt<'p> { CreateIndex { index, table, col }.into() }
  #[rule(Stmt -> AlterTable Id Drop Index Id)]
  fn alter_drop_index1(_: Token, table: &'p str, _: Token, _: Token, index: &'p str) -> Stmt<'p> { Stmt::DropIndex { index, table: Some(table) } }
  #[rule(Stmt -> AlterTable Id RenameTo Id)]
  fn alter_rename(_: Token, old: &'p str, _: Token, new: &'p str) -> Stmt<'p> { Stmt::Rename { old, new } }
  #[rule(Stmt -> AlterTable Id Add1 ForeignKey LPar Id RPar References Id LPar Id RPar)]
  fn alter_add_foreign(_: Token, table: &'p str, _: Token, _: Token, _: Token, col: &'p str, _: Token, _: Token, f_table: &'p str, _: Token, f_col: &'p str, _: Token) -> Stmt<'p> { AddForeign { table, col, f_table, f_col }.into() }
  #[rule(Stmt -> AlterTable Id Drop ForeignKey Id)]
  fn alter_drop_foreign(_: Token, table: &'p str, _: Token, _: Token, col: &'p str) -> Stmt<'p> { Stmt::DropForeign { table, col } }
  #[rule(Stmt -> AlterTable Id Add1 PrimaryKey LPar IdList RPar)]
  fn alter_add_primary(_: Token, table: &'p str, _: Token, _: Token, _: Token, cols: Vec<&'p str>, _: Token) -> Stmt<'p> { AddPrimary { table, cols }.into() }
  #[rule(Stmt -> AlterTable Id Drop PrimaryKey LPar IdList RPar)]
  fn alter_drop_primary(_: Token, table: &'p str, _: Token, _: Token, _: Token, cols: Vec<&'p str>, _: Token) -> Stmt<'p> { Stmt::DropPrimary { table, cols } }
  #[rule(Stmt -> AlterTable Id Add1 ColDecl)]
  fn alter_add_col(_: Token, table: &'p str, _: Token, col: ColDecl<'p>) -> Stmt<'p> { Stmt::AddCol { table, col } }
  #[rule(Stmt -> AlterTable Id Drop Id)]
  fn alter_drop_col(_: Token, table: &'p str, _: Token, col: &'p str) -> Stmt<'p> { Stmt::DropCol { table, col } }

  #[rule(WhereM -> Where CondList)]
  fn where_m1(_: Token, where_: Vec<Cond<'p>>) -> Vec<Cond<'p>> { where_ }
  #[rule(WhereM ->)]
  fn where_m0() -> Vec<Cond<'p>> { vec![] }

  #[rule(IdList -> Id)]
  fn id_list0(i: &'p str) -> Vec<&'p str> { vec![i] }
  #[rule(IdList -> IdList Comma Id)]
  fn id_list1(mut il: Vec<&'p str>, _: Token, i: &'p str) -> Vec<&'p str> { (il.push(i), il).1 }

  #[rule(AggList -> Agg)]
  fn agg_list0(a: Agg<'p>) -> Vec<Agg<'p>> { vec![a] }
  #[rule(AggList -> AggList Comma Agg)]
  fn agg_list1(mut al: Vec<Agg<'p>>, _: Token, a: Agg<'p>) -> Vec<Agg<'p>> { (al.push(a), al).1 }

  #[rule(LitList -> Lit)]
  fn lit_list0(l: CLit<'p>) -> Vec<CLit<'p>> { vec![l] }
  #[rule(LitList -> LitList Comma Lit)]
  fn lit_list1(mut ll: Vec<CLit<'p>>, _: Token, l: CLit<'p>) -> Vec<CLit<'p>> { (ll.push(l), ll).1 }

  #[rule(LitListList -> LPar LitList RPar)]
  fn lit_list_list0(_: Token, l: Vec<CLit<'p>>, _: Token) -> Vec<Vec<CLit<'p>>> { vec![l] }
  #[rule(LitListList -> LitListList Comma LPar LitList RPar)]
  fn lit_list_list1(mut ll: Vec<Vec<CLit<'p>>>, _: Token, _: Token, l: Vec<CLit<'p>>, _: Token) -> Vec<Vec<CLit<'p>>> { (ll.push(l), ll).1 }

  #[rule(Expr -> Atom)]
  fn expr_atom(a: Atom<'p>) -> Expr<'p> { Expr::Atom(a) }
  #[rule(Expr -> Sub Expr)]
  #[prec(UMinus)]
  fn expr_neg(_: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Sub, box (Expr::Atom(Atom::Lit(CLit::new(Lit::Number(0.0)))), r)) }
  #[rule(Expr -> Expr Add Expr)]
  fn expr_add(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Add, box (l, r)) }
  #[rule(Expr -> Expr Sub Expr)]
  fn expr_sub(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Sub, box (l, r)) }
  #[rule(Expr -> Expr Mul Expr)]
  fn expr_mul(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Mul, box (l, r)) }
  #[rule(Expr -> Expr Div Expr)]
  fn expr_div(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Div, box (l, r)) }
  #[rule(Expr -> Expr Mod Expr)]
  fn expr_mod(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Mod, box (l, r)) }
  #[rule(Expr -> LPar Expr RPar)]
  fn expr_par(_: Token, e: Expr<'p>, _: Token) -> Expr<'p> { e }
  #[rule(Expr -> Expr Lt Expr)]
  fn expr_lt(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Lt, box (l, r)) }
  #[rule(Expr -> Expr Le Expr)]
  fn expr_le(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Le, box (l, r)) }
  #[rule(Expr -> Expr Ge Expr)]
  fn expr_ge(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Ge, box (l, r)) }
  #[rule(Expr -> Expr Gt Expr)]
  fn expr_gt(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Gt, box (l, r)) }
  #[rule(Expr -> Expr Eq Expr)]
  fn expr_eq(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Eq, box (l, r)) }
  #[rule(Expr -> Expr Ne Expr)]
  fn expr_ne(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Ne, box (l, r)) }
  #[rule(Expr -> Expr And Expr)]
  fn expr_and(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::And(box (l, r)) }
  #[rule(Expr -> Expr Or Expr)]
  fn expr_or(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Or(box (l, r)) }
  #[rule(Expr -> Expr Is Null)]
  fn expr_is_null(e: Expr<'p>, _: Token, _: Token) -> Expr<'p> { Expr::Null(box e, true) }
  #[rule(Expr -> Expr Is NotNull)]
  fn expr_is_not_null(e: Expr<'p>, _: Token, _: Token) -> Expr<'p> { Expr::Null(box e, false) }
  #[rule(Expr -> Expr Like StrLit)]
  fn expr_like(&self, e: Expr<'p>, _: Token, s: Token) -> Expr<'p> { Expr::Like(box e, self.escape(s.str_trim())) }

  #[rule(SetList -> Id Eq Expr)]
  fn set_list0(col: &'p str, _: Token, l: Expr<'p>) -> Vec<(&'p str, Expr<'p>)> { vec![(col, l)] }
  #[rule(SetList -> SetList Comma Id Eq Expr)]
  fn set_list1(mut sl: Vec<(&'p str, Expr<'p>)>, _: Token, col: &'p str, _: Token, r: Expr<'p>) -> Vec<(&'p str, Expr<'p>)> { (sl.push((col, r)), sl).1 }

  #[rule(FieldList -> ColDecl)]
  fn field_list0(c: ColDecl<'p>) -> FieldList<'p> { (vec![c], vec![]) }
  #[rule(FieldList -> ColCons)]
  fn field_list1(c: ColCons<'p>) -> FieldList<'p> { (vec![], vec![c]) }
  #[rule(FieldList -> FieldList Comma ColDecl)]
  fn field_list2(mut fl: FieldList<'p>, _: Token, c: ColDecl<'p>) -> FieldList<'p> { (fl.0.push(c), fl).1 }
  #[rule(FieldList -> FieldList Comma ColCons)]
  fn field_list3(mut fl: FieldList<'p>, _: Token, c: ColCons<'p>) -> FieldList<'p> { (fl.1.push(c), fl).1 }

  #[rule(ColDecl -> Id ColTy)]
  fn field0(col: &'p str, ty: ColTy) -> ColDecl<'p> { ColDecl { col, ty, notnull: false, dft: None } }
  #[rule(ColDecl -> Id ColTy NotNull)]
  fn field1(col: &'p str, ty: ColTy, _: Token) -> ColDecl<'p> { ColDecl { col, ty, notnull: true, dft: None } }
  #[rule(ColDecl -> Id ColTy Default Lit)]
  fn field2(col: &'p str, ty: ColTy, _: Token, dft: CLit<'p>) -> ColDecl<'p> { ColDecl { col, ty, notnull: false, dft: Some(dft) } }
  #[rule(ColDecl -> Id ColTy NotNull Default Lit)]
  fn field3(col: &'p str, ty: ColTy, _: Token, _: Token, dft: CLit<'p>) -> ColDecl<'p> { ColDecl { col, ty, notnull: true, dft: Some(dft) } }
  #[rule(ColCons -> ForeignKey LPar Id RPar References Id LPar Id RPar)]
  fn field5(_: Token, _: Token, col: &'p str, _: Token, _: Token, f_table: &'p str, _: Token, f_col: &'p str, _: Token) -> ColCons<'p> { ColCons::Foreign { col, f_table, f_col } }
  #[rule(ColCons -> PrimaryKey LPar IdList RPar)]
  fn field6(_: Token, _: Token, il: Vec<&'p str>, _: Token) -> ColCons<'p> { ColCons::Primary(il) }
  #[rule(ColCons -> Unique LPar Id RPar)]
  fn field7(_: Token, _: Token, col: &'p str, _: Token) -> ColCons<'p> { ColCons::Unique(col) }
  #[rule(ColCons -> Check LPar Id In LPar LitList RPar RPar)]
  fn field8(_: Token, _: Token, col: &'p str, _: Token, _: Token, ll: Vec<CLit<'p>>, _: Token, _: Token) -> ColCons<'p> { ColCons::Check(col, ll) }

  #[rule(Agg -> ColRef)]
  fn agg0(col: ColRef<'p>) -> Agg<'p> { Agg { col, op: None } }
  #[rule(Agg -> Avg LPar ColRef RPar)]
  fn agg_avg(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: Some(Avg) } }
  #[rule(Agg -> Sum LPar ColRef RPar)]
  fn agg_sum(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: Some(Sum) } }
  #[rule(Agg -> Min LPar ColRef RPar)]
  fn agg_min(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: Some(Min) } }
  #[rule(Agg -> Max LPar ColRef RPar)]
  fn agg_max(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: Some(Max) } }
  #[rule(Agg -> Count LPar ColRef RPar)]
  fn agg_count(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: Some(Count) } }
  // for CountAll, `col` is not accessible (for compatibility, `col` is not defined as Option<ColRef>)
  // "*" is just for the convenience of printing
  #[rule(Agg -> Count LPar Mul RPar)]
  fn agg_count_all(_: Token, _: Token, _: Token, _: Token) -> Agg<'p> { Agg { col: ColRef { table: None, col: "*" }, op: Some(CountAll) } }

  #[rule(ColRef -> Id)]
  fn col_ref0(col: &'p str) -> ColRef<'p> { ColRef { table: None, col } }
  #[rule(ColRef -> Id Dot Id)]
  fn col_ref1(table: &'p str, _: Token, col: &'p str) -> ColRef<'p> { ColRef { table: Some(table), col } }

  #[rule(CondList -> CondList And Cond)]
  fn where1(mut cl: Vec<Cond<'p>>, _: Token, c: Cond<'p>) -> Vec<Cond<'p>> { (cl.push(c), cl).1 }
  #[rule(CondList -> Cond)]
  fn where0(c: Cond<'p>) -> Vec<Cond<'p>> { vec![c] }

  #[rule(Cond -> ColRef Lt Atom)]
  fn cond_lt(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Cond<'p> { Cond::Cmp(Lt, l, r) }
  #[rule(Cond -> ColRef Le Atom)]
  fn cond_le(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Cond<'p> { Cond::Cmp(Le, l, r) }
  #[rule(Cond -> ColRef Ge Atom)]
  fn cond_ge(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Cond<'p> { Cond::Cmp(Ge, l, r) }
  #[rule(Cond -> ColRef Gt Atom)]
  fn cond_gt(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Cond<'p> { Cond::Cmp(Gt, l, r) }
  #[rule(Cond -> ColRef Eq Atom)]
  fn cond_eq(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Cond<'p> { Cond::Cmp(Eq, l, r) }
  #[rule(Cond -> ColRef Ne Atom)]
  fn cond_ne(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Cond<'p> { Cond::Cmp(Ne, l, r) }
  #[rule(Cond -> ColRef Is Null)]
  fn cond_is_null(c: ColRef<'p>, _: Token, _: Token) -> Cond<'p> { Cond::Null(c, true) }
  #[rule(Cond -> ColRef Is NotNull)]
  fn cond_is_not_null(c: ColRef<'p>, _: Token, _: Token) -> Cond<'p> { Cond::Null(c, false) }
  #[rule(Cond -> ColRef Like StrLit)]
  fn cond_like(c: ColRef<'p>, _: Token, s: Token) -> Cond<'p> { Cond::Like(c, s.str_trim()) }

  #[rule(Atom -> ColRef)]
  fn atom_col_ref(c: ColRef<'p>) -> Atom<'p> { Atom::ColRef(c) }
  #[rule(Atom -> Lit)]
  fn atom_lit(l: CLit<'p>) -> Atom<'p> { Atom::Lit(l) }

  #[rule(Lit -> Null)]
  fn lit_null(_: Token) -> CLit<'p> { CLit::new(Lit::Null) }
  #[rule(Lit -> True)]
  fn lit_true(_: Token) -> CLit<'p> { CLit::new(Lit::Bool(true)) }
  #[rule(Lit -> False)]
  fn lit_false(_: Token) -> CLit<'p> { CLit::new(Lit::Bool(false)) }
  #[rule(Lit -> IntLit)]
  fn lit_int(&mut self, t: Token) -> CLit<'p> { t.parse(|x: i32| CLit::new(Lit::Number(x as f64)), |line, col, s| self.pe.push(PE { line, col, kind: InvalidInt(s) })) }
  #[rule(Lit -> FloatLit)]
  fn lit_float(&mut self, t: Token) -> CLit<'p> { t.parse(|x: f32| CLit::new(Lit::Number(x as f64)), |line, col, s| self.pe.push(PE { line, col, kind: InvalidFloat(s) })) }
  #[rule(Lit -> StrLit)]
  fn lit_str(t: Token) -> CLit<'p> { CLit::new(Lit::Str(t.str_trim())) }

  #[rule(BareTy -> Bool)]
  fn bare_ty_bool(_: Token) -> BareTy { Bool }
  #[rule(BareTy -> Int)]
  fn bare_ty_int(_: Token) -> BareTy { Int }
  #[rule(BareTy -> Float)]
  fn bare_ty_float(_: Token) -> BareTy { Float }
  #[rule(BareTy -> Date)]
  fn bare_ty_date(_: Token) -> BareTy { Date }
  #[rule(BareTy -> Char)]
  fn bare_ty_var_char(_: Token) -> BareTy { Char }

  #[rule(ColTy -> BareTy LPar IntLit RPar)]
  fn col_ty(&mut self, ty: BareTy, _: Token, t: Token, _: Token) -> ColTy { t.parse(|size| ColTy::FixTy(FixTy { size, ty }), |line, col, s| self.pe.push(PE { line, col, kind: InvalidTypeSize(s) })) }
  #[rule(ColTy -> Varchar LPar IntLit RPar)]
  fn col_ty_varchar(&mut self, _: Token, _: Token, t: Token, _: Token) -> ColTy { t.parse(|size| ColTy::Varchar(size), |line, col, s| self.pe.push(PE { line, col, kind: InvalidTypeSize(s) })) }
  #[rule(ColTy -> Bool)]
  fn col_ty_bool(_: Token) -> ColTy { ColTy::FixTy(FixTy { size: 0, ty: Bool }) }
  #[rule(ColTy -> Int)]
  fn col_ty_int(_: Token) -> ColTy { ColTy::FixTy(FixTy { size: 0, ty: Int }) }
  #[rule(ColTy -> Float)]
  fn col_ty_float(_: Token) -> ColTy { ColTy::FixTy(FixTy { size: 0, ty: Float }) }
  #[rule(ColTy -> Date)]
  fn col_ty_date(_: Token) -> ColTy { ColTy::FixTy(FixTy { size: 0, ty: Date }) }
}