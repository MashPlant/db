use parser_macros::lalr1;
use common::{ColTy, BareTy::{*, self}, Lit};
use crate::ast::*;

pub struct Parser;

impl<'p> Token<'p> {
  fn str_trim(&self) -> &'p str { std::str::from_utf8(&self.piece[1..self.piece.len() - 1]).unwrap() }
  fn str(&self) -> &'p str { std::str::from_utf8(self.piece).unwrap() }
  fn i32(&self) -> i32 { self.str().parse().unwrap() }
  fn u8(&self) -> u8 { self.str().parse().unwrap() }
  fn f32(&self) -> f32 { self.str().parse().unwrap() }
}

#[lalr1(Program)]
#[lex(r##"
priority = []

[lexical] # I hate sql...
'(c|C)(r|R)(e|E)(a|A)(t|T)(e|E)' = 'Create'
'(d|D)(r|R)(o|O)(p|P)' = 'Drop'
'(u|U)(s|S)(e|E)' = 'Use'
'(s|S)(h|H)(o|O)(w|W)' = 'Show'
'(d|D)(e|E)(s|S)(c|C)' = 'Desc'
'(d|D)(a|A)(t|T)(a|A)(b|B)(a|A)(s|S)(e|E)(s|S)' = 'DataBases'
'(d|D)(a|A)(t|T)(a|A)(b|B)(a|A)(s|S)(e|E)' = 'DataBase'
'(t|T)(a|A)(b|B)(l|L)(e|E)(s|S)' = 'Tables'
'(t|T)(a|A)(b|B)(l|L)(e|E)' = 'Table'
'(s|S)(e|E)(l|L)(e|E)(c|C)(t|T)' = 'Select'
'(d|D)(e|E)(l|L)(e|E)(t|T)(e|E)' = 'Delete'
'(i|I)(n|N)(s|S)(e|E)(r|R)(t|T)' = 'Insert'
'(u|U)(p|P)(d|D)(a|A)(t|T)(e|E)' = 'Update'
'(v|V)(a|A)(l|L)(u|U)(e|E)(s|S)' = 'Values'
'(r|R)(e|E)(f|F)(e|E)(r|R)(e|E)(n|N)(c|C)(e|E)(s|S)' = 'References'
'(s|S)(e|E)(t|T)' = 'Set'
'(f|F)(r|R)(o|O)(m|M)' = 'From'
'(i|I)(n|N)(t|T)(o|O)' = 'Into'
'(w|W)(h|H)(e|E)(r|R)(e|E)' = 'Where'
'(s|S)(u|U)(m|M)' = 'Sum'
'(a|A)(v|V)(g|G)' = 'Avg'
'(m|M)(i|I)(n|N)' = 'Min'
'(m|M)(a|A)(x|X)' = 'Max'
'(n|N)(o|O)(t|T)\s+(n|N)(u|U)(l|L)(l|L)' = 'NotNull'
'(p|P)(r|R)(i|I)(m|M)(a|A)(r|R)(y|Y)\s+(k|K)(e|E)(y|Y)' = 'PrimaryKey'
'(f|F)(o|O)(r|R)(e|E)(i|I)(g|G)(n|N)\s+(k|K)(e|E)(y|Y)' = 'ForeignKey'
'(l|L)(i|I)(k|K)(e|E)' = 'Like'
'(i|I)(n|N)(d|D)(e|E)(x|X)' = 'Index'
'(c|C)(h|H)(e|E)(c|C)(k|K)' = 'Check'
'(i|I)(n|N)' = 'In'
'(i|I)(s|S)' = 'Is'
'(i|I)(n|N)(t|T)' = 'Int'
'(b|B)(o|O)(o|O)(l|L)' = 'Bool'
'(c|C)(h|H)(a|A)(r|R)' = 'Char'
'(v|V)(a|A)(r|R)(c|C)(h|H)(a|A)(r|R)' = 'VarChar'
'(f|F)(l|L)(o|O)(a|A)(t|T)' = 'Float'
'(d|D)(a|A)(t|T)(e|E)' = 'Date'
'(n|N)(u|U)(l|L)(l|L)' = 'Null'
'(t|T)(r|R)(u|U)(e|E)' = 'True'
'(f|F)(a|A)(l|L)(s|S)(e|E)' = 'False'
'(a|A)(n|N)(d|D)' = 'And'
'(n|N)(o|O)(t|T)' = 'Not'
'<' = 'Lt'
'<=' = 'Le'
'>=' = 'Ge'
'>' = 'Gt'
'=' = 'Eq'
'<>' = 'Ne'
'\*' = 'Mul'
'\.' = 'Dot'
',' = 'Comma'
';' = 'Semicolon'
'\(' = 'LParen'
'\)' = 'RParen'
'--[^\n]*' = '_Eps'
'\s+' = '_Eps'
'\d+\.\d*' = 'FloatLit'
'(\d+)|(-\d+)' = 'IntLit'
"'[^'\\\\]*(\\\\.[^'\\\\]*)*'" = 'StrLit'
'[A-Za-z][_0-9A-Za-z]*' = 'Id'
"##)]
impl<'p> Parser {
  #[rule(Program ->)]
  fn stmt_list0() -> Vec<Stmt<'p>> { vec![] }
  #[rule(Program -> Program Stmt Semicolon)]
  fn stmt_list1(mut sl: Vec<Stmt<'p>>, s: Stmt<'p>, _: Token) -> Vec<Stmt<'p>> { (sl.push(s), sl).1 }

  #[rule(Stmt -> Show DataBases)]
  fn stmt_show_dbs(_: Token, _: Token) -> Stmt<'p> { Stmt::ShowDbs }
  #[rule(Stmt -> Show DataBase Id)]
  fn stmt_show_db(_: Token, _: Token, t: Token) -> Stmt<'p> { Stmt::ShowDb(t.str()) }
  #[rule(Stmt -> Create DataBase Id)]
  fn stmt_create_db(_: Token, _: Token, t: Token) -> Stmt<'p> { Stmt::CreateDb(t.str()) }
  #[rule(Stmt -> Drop DataBase Id)]
  fn stmt_drop_db(_: Token, _: Token, t: Token) -> Stmt<'p> { Stmt::DropDb(t.str()) }
  #[rule(Stmt -> Use Id)]
  fn stmt_use_db(_: Token, t: Token) -> Stmt<'p> { Stmt::UseDb(t.str()) }
  #[rule(Stmt -> Drop Table Id)]
  fn stmt_drop_table(_: Token, _: Token, t: Token) -> Stmt<'p> { Stmt::DropTable(t.str()) }
  #[rule(Stmt -> Create Index Id LParen Id RParen)]
  fn stmt_create_index(_: Token, _: Token, t: Token, _: Token, c: Token, _: Token) -> Stmt<'p> {
    Stmt::CreateIndex(t.str(), c.str())
  }
  #[rule(Stmt -> Drop Index Id LParen Id RParen)]
  fn stmt_drop_index(_: Token, _: Token, t: Token, _: Token, c: Token, _: Token) -> Stmt<'p> {
    Stmt::DropIndex(t.str(), c.str())
  }
  #[rule(Stmt -> Create Table Id LParen ColDeclList ConsListM RParen)]
  fn stmt_create_table(_: Token, _: Token, t: Token, _: Token, cols: Vec<ColDecl<'p>>, cons: Vec<TableCons<'p>>, _: Token) -> Stmt<'p> {
    Stmt::CreateTable(CreateTable { name: t.str(), cols, cons })
  }
  #[rule(Stmt -> Show Tables)]
  fn stmt_show_tables(_: Token, _: Token) -> Stmt<'p> { Stmt::ShowTables }
  #[rule(Stmt -> Desc Id)]
  fn stmt_show_table(_: Token, t: Token) -> Stmt<'p> { Stmt::ShowTable(t.str()) }
  #[rule(Stmt -> Select Mul From IdList Where WhereList)]
  fn stmt_select0(_: Token, _: Token, _: Token, tables: Vec<&'p str>, _: Token, where_: Vec<Expr<'p>>) -> Stmt<'p> {
    Stmt::Select(Select { ops: None, tables, where_ })
  }
  #[rule(Stmt -> Select AggList From IdList Where WhereList)]
  fn stmt_select1(_: Token, ops: Vec<Agg<'p>>, _: Token, tables: Vec<&'p str>, _: Token, where_: Vec<Expr<'p>>) -> Stmt<'p> {
    Stmt::Select(Select { ops: Some(ops), tables, where_ })
  }
  #[rule(Stmt -> Insert Into Id Values LitListList)]
  fn stmt_insert(_: Token, _: Token, t: Token, _: Token, vals: Vec<Vec<Lit<'p>>>) -> Stmt<'p> {
    Stmt::Insert(Insert { table: t.str(), vals })
  }
  #[rule(Stmt -> Update Id Set SetList WhereList)]
  fn stmt_update(_: Token, t: Token, _: Token, sets: Vec<(&'p str, Lit<'p>)>, where_: Vec<Expr<'p>>) -> Stmt<'p> {
    Stmt::Update(Update { table: t.str(), sets, where_ })
  }
  #[rule(Stmt -> Delete From Id WhereList)]
  fn stmt_delete(_: Token, _: Token, t: Token, where_: Vec<Expr<'p>>) -> Stmt<'p> {
    Stmt::Delete(Delete { table: t.str(), where_ })
  }

  #[rule(ConsListM ->)]
  fn cons_list_m0() -> Vec<TableCons<'p>> { vec![] }
  #[rule(ConsListM -> Comma ConsList)]
  fn cons_list_m1(_: Token, cl: Vec<TableCons<'p>>) -> Vec<TableCons<'p>> { cl }

  #[rule(NotNullM ->)]
  fn not_null_m0() -> bool { false }
  #[rule(NotNullM -> NotNull)]
  fn not_null_m1(_: Token) -> bool { true }

  #[rule(IdList -> Id)]
  fn id_list0(t: Token) -> Vec<&'p str> { vec![t.str()] }
  #[rule(IdList -> IdList Comma Id)]
  fn id_list1(mut il: Vec<&'p str>, _: Token, t: Token) -> Vec<&'p str> { (il.push(t.str()), il).1 }

  #[rule(AggList -> Agg)]
  fn agg_list0(a: Agg<'p>) -> Vec<Agg<'p>> { vec![a] }
  #[rule(AggList -> AggList Comma Agg)]
  fn agg_list1(mut al: Vec<Agg<'p>>, _: Token, a: Agg<'p>) -> Vec<Agg<'p>> { (al.push(a), al).1 }

  #[rule(LitList -> Lit)]
  fn lit_list0(l: Lit<'p>) -> Vec<Lit<'p>> { vec![l] }
  #[rule(LitList -> LitList Comma Lit)]
  fn lit_list1(mut ll: Vec<Lit<'p>>, _: Token, l: Lit<'p>) -> Vec<Lit<'p>> { (ll.push(l), ll).1 }

  #[rule(LitListList -> LParen LitList RParen)]
  fn lit_list_list0(_: Token, l: Vec<Lit<'p>>, _: Token) -> Vec<Vec<Lit<'p>>> { vec![l] }
  #[rule(LitListList -> LitListList Comma LParen LitList RParen)]
  fn lit_list_list1(mut ll: Vec<Vec<Lit<'p>>>, _: Token, _: Token, l: Vec<Lit<'p>>, _: Token) -> Vec<Vec<Lit<'p>>> {
    (ll.push(l), ll).1
  }

  #[rule(SetList -> Id Eq Lit)]
  fn set_list0(t: Token, _: Token, l: Lit<'p>) -> Vec<(&'p str, Lit<'p>)> { vec![(t.str(), l)] }
  #[rule(SetList -> SetList Comma Id Eq Lit)]
  fn set_list1(mut sl: Vec<(&'p str, Lit<'p>)>, _: Token, t: Token, _: Token, l: Lit<'p>) -> Vec<(&'p str, Lit<'p>)> {
    (sl.push((t.str(), l)), sl).1
  }

  #[rule(ColDeclList -> Id ColTy NotNullM)]
  fn col_decl_list0(t: Token, ty: ColTy, notnull: bool) -> Vec<ColDecl<'p>> { vec![ColDecl { name: t.str(), ty, notnull }] }
  #[rule(ColDeclList -> ColDeclList Comma Id ColTy NotNullM)]
  fn col_decl_list1(mut cl: Vec<ColDecl<'p>>, _: Token, t: Token, ty: ColTy, notnull: bool) -> Vec<ColDecl<'p>> {
    (cl.push(ColDecl { name: t.str(), ty, notnull }), cl).1
  }

  #[rule(ConsList -> Cons)]
  fn cons_list0(c: Vec<TableCons<'p>>) -> Vec<TableCons<'p>> { c }
  #[rule(ConsList -> ConsList Comma Cons)]
  fn cons_list1(mut cl: Vec<TableCons<'p>>, _: Token, mut c: Vec<TableCons<'p>>) -> Vec<TableCons<'p>> {
    (cl.append(&mut c), cl).1
  }

  #[rule(Cons -> ForeignKey LParen Id RParen References Id LParen Id RParen)]
  fn cons_foreign(_: Token, _: Token, t: Token, _: Token, _: Token, table: Token, _: Token, col: Token, _: Token) -> Vec<TableCons<'p>> {
    vec![TableCons { name: t.str(), kind: TableConsKind::Foreign { table: table.str(), col: col.str() } }]
  }
  #[rule(Cons -> PrimaryKey LParen IdList RParen)]
  fn cons_primary(_: Token, _: Token, il: Vec<&'p str>, _: Token) -> Vec<TableCons<'p>> {
    il.into_iter().map(|name| TableCons { name, kind: TableConsKind::Primary }).collect()
  }
  #[rule(Cons -> Check LParen Id In LParen LitList RParen RParen)]
  fn cons_check(_: Token, _: Token, t: Token, _: Token, _: Token, ll: Vec<Lit<'p>>, _: Token, _: Token) -> Vec<TableCons<'p>> {
    vec![TableCons { name: t.str(), kind: TableConsKind::Check(ll) }]
  }

  #[rule(Agg -> ColRef)]
  fn agg0(col: ColRef<'p>) -> Agg<'p> { Agg { col, op: AggOp::None } }
  #[rule(Agg -> Avg LParen ColRef RParen)]
  fn agg_avg(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: AggOp::Avg } }
  #[rule(Agg -> Sum LParen ColRef RParen)]
  fn agg_sum(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: AggOp::Sum } }
  #[rule(Agg -> Min LParen ColRef RParen)]
  fn agg_min(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: AggOp::Min } }
  #[rule(Agg -> Max LParen ColRef RParen)]
  fn agg_max(_: Token, _: Token, col: ColRef<'p>, _: Token) -> Agg<'p> { Agg { col, op: AggOp::Max } }

  #[rule(ColRef -> Id)]
  fn col_ref0(c: Token) -> ColRef<'p> { ColRef { table: None, col: c.str() } }
  #[rule(ColRef -> Id Dot Id)]
  fn col_ref1(t: Token, _: Token, c: Token) -> ColRef<'p> { ColRef { table: Some(t.str()), col: c.str() } }

  #[rule(WhereList -> WhereList And Expr)]
  fn where1(mut wl: Vec<Expr<'p>>, _: Token, e: Expr<'p>) -> Vec<Expr<'p>> { (wl.push(e), wl).1 }
  #[rule(WhereList -> Expr)]
  fn where0(e: Expr<'p>) -> Vec<Expr<'p>> { vec![e] }

  #[rule(Expr -> ColRef Lt Atom)]
  fn expr_lt(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Expr<'p> { Expr::Cmp(CmpOp::Lt, l, r) }
  #[rule(Expr -> ColRef Le Atom)]
  fn expr_le(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Expr<'p> { Expr::Cmp(CmpOp::Le, l, r) }
  #[rule(Expr -> ColRef Ge Atom)]
  fn expr_ge(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Expr<'p> { Expr::Cmp(CmpOp::Ge, l, r) }
  #[rule(Expr -> ColRef Gt Atom)]
  fn expr_gt(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Expr<'p> { Expr::Cmp(CmpOp::Gt, l, r) }
  #[rule(Expr -> ColRef Eq Atom)]
  fn expr_eq(l: ColRef<'p>, _: Token, r: Atom<'p>) -> Expr<'p> { Expr::Cmp(CmpOp::Eq, l, r) }
  #[rule(Expr -> ColRef Is Null)]
  fn expr_is_null(c: ColRef<'p>, _: Token, _: Token) -> Expr<'p> { Expr::Null(c, true) }
  #[rule(Expr -> ColRef Is NotNull)]
  fn expr_is_not_null(c: ColRef<'p>, _: Token, _: Token) -> Expr<'p> { Expr::Null(c, false) }
  #[rule(Expr -> ColRef Like StrLit)]
  fn expr_like(c: ColRef<'p>, _: Token, s: Token) -> Expr<'p> { Expr::Like(c, s.str()) }

  #[rule(Atom -> ColRef)]
  fn atom_col_ref(c: ColRef<'p>) -> Atom<'p> { Atom::ColRef(c) }
  #[rule(Atom -> Lit)]
  fn atom_lit(l: Lit<'p>) -> Atom<'p> { Atom::Lit(l) }

  #[rule(Lit -> Null)]
  fn lit_null(_: Token) -> Lit<'p> { Lit::Null }
  #[rule(Lit -> IntLit)]
  fn lit_int(t: Token) -> Lit<'p> { Lit::Int(t.i32()) }
  #[rule(Lit -> True)]
  fn lit_true(_: Token) -> Lit<'p> { Lit::Bool(true) }
  #[rule(Lit -> False)]
  fn lit_false(_: Token) -> Lit<'p> { Lit::Bool(false) }
  #[rule(Lit -> FloatLit)]
  fn lit_float(t: Token) -> Lit<'p> { Lit::Float(t.f32()) }
  #[rule(Lit -> StrLit)]
  fn lit_str(t: Token) -> Lit<'p> { Lit::Str(t.str_trim()) }

  #[rule(BareTy -> Int)]
  fn bare_ty_int(_: Token) -> BareTy { Int }
  #[rule(BareTy -> Bool)]
  fn bare_ty_bool(_: Token) -> BareTy { Bool }
  #[rule(BareTy -> Float)]
  fn bare_ty_float(_: Token) -> BareTy { Float }
  #[rule(BareTy -> Char)]
  fn bare_ty_char(_: Token) -> BareTy { Char }
  #[rule(BareTy -> VarChar)]
  fn bare_ty_var_char(_: Token) -> BareTy { VarChar }
  #[rule(BareTy -> Date)]
  fn bare_ty_date(_: Token) -> BareTy { Date }

  #[rule(ColTy -> BareTy LParen IntLit RParen)]
  fn col_ty(ty: BareTy, _: Token, t: Token, _: Token) -> ColTy { ColTy { size: t.u8(), ty } }
  #[rule(ColTy -> Int)]
  fn col_ty_int(_: Token) -> ColTy { ColTy { size: 0, ty: Int } }
  #[rule(ColTy -> Bool)]
  fn col_ty_bool(_: Token) -> ColTy { ColTy { size: 0, ty: Bool } }
  #[rule(ColTy -> Float)]
  fn col_ty_float(_: Token) -> ColTy { ColTy { size: 0, ty: Float } }
  #[rule(ColTy -> Date)]
  fn col_ty_date(_: Token) -> ColTy { ColTy { size: 0, ty: Date } }
}