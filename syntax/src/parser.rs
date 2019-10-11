use common::{BareTy::{*, self}, ParserError as PE, ParserErrorKind::*, Lit, CLit, ColTy, AggOp::*, BinOp::*, CmpOp::*};
use crate::ast::*;

#[derive(Default)]
pub struct Parser<'a>(pub Vec<PE<'a>>);

impl<'p> Token<'p> {
  fn str_trim(&self) -> &'p str { std::str::from_utf8(&self.piece[1..self.piece.len() - 1]).unwrap() }
  fn str(&self) -> &'p str { std::str::from_utf8(self.piece).unwrap() }
}

#[parser_macros::lalr1(Program)]
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
'(c|C)(o|O)(u|U)(n|N)(t|T)' = 'Count'
'(n|N)(o|O)(t|T)\s+(n|N)(u|U)(l|L)(l|L)' = 'NotNull'
'(p|P)(r|R)(i|I)(m|M)(a|A)(r|R)(y|Y)' = 'Primary'
'(f|F)(o|O)(r|R)(e|E)(i|I)(g|G)(n|N)' = 'Foreign'
'(u|U)(n|N)(i|I)(q|Q)(u|U)(e|E)' = 'Unique'
'(k|K)(e|E)(y|Y)' = 'Key'
'(l|L)(i|I)(k|K)(e|E)' = 'Like'
'(i|I)(n|N)(d|D)(e|E)(x|X)' = 'Index'
'(c|C)(h|H)(e|E)(c|C)(k|K)' = 'Check'
'(i|I)(n|N)' = 'In'
'(i|I)(s|S)' = 'Is'
'(i|I)(n|N)(t|T)' = 'Int'
'(b|B)(o|O)(o|O)(l|L)' = 'Bool'
'(c|C)(h|H)(a|A)(r|R)' = 'VarChar' # we won't handle char specially, just use varchar
'(v|V)(a|A)(r|R)(c|C)(h|H)(a|A)(r|R)' = 'VarChar'
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
'(<>)(!=)' = 'Ne'
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
'[A-Za-z]\w*' = 'Id'
'.' = '_Err'
"##)]
impl<'p> Parser<'p> {
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
  #[rule(Stmt -> Create Index Id LPar Id RPar)]
  fn stmt_create_index(_: Token, _: Token, t: Token, _: Token, c: Token, _: Token) -> Stmt<'p> { Stmt::CreateIndex { table: t.str(), col: c.str() } }
  #[rule(Stmt -> Drop Index Id LPar Id RPar)]
  fn stmt_drop_index(_: Token, _: Token, t: Token, _: Token, c: Token, _: Token) -> Stmt<'p> { Stmt::DropIndex { table: t.str(), col: c.str() } }
  #[rule(Stmt -> Create Table Id LPar ColDeclList ConsListM RPar)]
  fn stmt_create_table(_: Token, _: Token, t: Token, _: Token, cols: Vec<ColDecl<'p>>, cons: Vec<TableCons<'p>>, _: Token) -> Stmt<'p> { Stmt::CreateTable(CreateTable { name: t.str(), cols, cons }) }
  #[rule(Stmt -> Show Tables)]
  fn stmt_show_tables(_: Token, _: Token) -> Stmt<'p> { Stmt::ShowTables }
  #[rule(Stmt -> Desc Id)]
  fn stmt_show_table(_: Token, t: Token) -> Stmt<'p> { Stmt::ShowTable(t.str()) }
  #[rule(Stmt -> Select Mul From IdList WhereM)]
  fn stmt_select0(_: Token, _: Token, _: Token, tables: Vec<&'p str>, where_: Vec<Cond<'p>>) -> Stmt<'p> { Stmt::Select(Select { ops: None, tables, where_ }) }
  #[rule(Stmt -> Select AggList From IdList WhereM)]
  fn stmt_select1(_: Token, ops: Vec<Agg<'p>>, _: Token, tables: Vec<&'p str>, where_: Vec<Cond<'p>>) -> Stmt<'p> { Stmt::Select(Select { ops: Some(ops), tables, where_ }) }
  #[rule(Stmt -> Insert Into Id Values LitListList)]
  fn stmt_insert(_: Token, _: Token, t: Token, _: Token, vals: Vec<Vec<CLit<'p>>>) -> Stmt<'p> { Stmt::Insert(Insert { table: t.str(), vals }) }
  #[rule(Stmt -> Update Id Set SetList WhereM)]
  fn stmt_update(_: Token, t: Token, _: Token, sets: Vec<(&'p str, Expr<'p>)>, where_: Vec<Cond<'p>>) -> Stmt<'p> { Stmt::Update(Update { table: t.str(), sets, where_ }) }
  #[rule(Stmt -> Delete From Id WhereM)]
  fn stmt_delete(_: Token, _: Token, t: Token, where_: Vec<Cond<'p>>) -> Stmt<'p> { Stmt::Delete(Delete { table: t.str(), where_ }) }

  #[rule(WhereM -> Where WhereList)]
  fn where_m1(_: Token, where_: Vec<Cond<'p>>) -> Vec<Cond<'p>> { where_ }
  #[rule(WhereM ->)]
  fn where_m0() -> Vec<Cond<'p>> { vec![] }

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
  fn expr_neg(_: Token, e: Expr<'p>) -> Expr<'p> { Expr::Neg(Box::new(e)) }
  #[rule(Expr -> Expr Add Expr)]
  fn expr_add(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Add, Box::new((l, r))) }
  #[rule(Expr -> Expr Sub Expr)]
  fn expr_sub(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Sub, Box::new((l, r))) }
  #[rule(Expr -> Expr Mul Expr)]
  fn expr_mul(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Mul, Box::new((l, r))) }
  #[rule(Expr -> Expr Div Expr)]
  fn expr_div(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Div, Box::new((l, r))) }
  #[rule(Expr -> Expr Mod Expr)]
  fn expr_mod(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Bin(Mod, Box::new((l, r))) }
  #[rule(Expr -> LPar Expr RPar)]
  fn expr_par(_: Token, e: Expr<'p>, _: Token) -> Expr<'p> { e }
  #[rule(Expr -> Expr Lt Expr)]
  fn expr_lt(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Lt, Box::new((l, r))) }
  #[rule(Expr -> Expr Le Expr)]
  fn expr_le(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Le, Box::new((l, r))) }
  #[rule(Expr -> Expr Ge Expr)]
  fn expr_ge(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Ge, Box::new((l, r))) }
  #[rule(Expr -> Expr Gt Expr)]
  fn expr_gt(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Gt, Box::new((l, r))) }
  #[rule(Expr -> Expr Eq Expr)]
  fn expr_eq(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Eq, Box::new((l, r))) }
  #[rule(Expr -> Expr Ne Expr)]
  fn expr_ne(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Cmp(Ne, Box::new((l, r))) }
  #[rule(Expr -> Expr And Expr)]
  fn expr_and(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::And(Box::new((l, r))) }
  #[rule(Expr -> Expr Or Expr)]
  fn expr_or(l: Expr<'p>, _: Token, r: Expr<'p>) -> Expr<'p> { Expr::Or(Box::new((l, r))) }
  #[rule(Expr -> Expr Is Null)]
  fn expr_is_null(e: Expr<'p>, _: Token, _: Token) -> Expr<'p> { Expr::Null(Box::new(e), true) }
  #[rule(Expr -> Expr Is NotNull)]
  fn expr_is_not_null(e: Expr<'p>, _: Token, _: Token) -> Expr<'p> { Expr::Null(Box::new(e), false) }
  #[rule(Expr -> Expr Like StrLit)]
  fn expr_like(e: Expr<'p>, _: Token, s: Token) -> Expr<'p> { Expr::Like(Box::new(e), s.str_trim()) }

  #[rule(SetList -> Id Eq Expr)]
  fn set_list0(t: Token, _: Token, l: Expr<'p>) -> Vec<(&'p str, Expr<'p>)> { vec![(t.str(), l)] }
  #[rule(SetList -> SetList Comma Id Eq Expr)]
  fn set_list1(mut sl: Vec<(&'p str, Expr<'p>)>, _: Token, t: Token, _: Token, l: Expr<'p>) -> Vec<(&'p str, Expr<'p>)> { (sl.push((t.str(), l)), sl).1 }

  #[rule(ColDeclList -> Id ColTy NotNullM)]
  fn col_decl_list0(t: Token, ty: ColTy, notnull: bool) -> Vec<ColDecl<'p>> { vec![ColDecl { name: t.str(), ty, notnull }] }
  #[rule(ColDeclList -> ColDeclList Comma Id ColTy NotNullM)]
  fn col_decl_list1(mut cl: Vec<ColDecl<'p>>, _: Token, t: Token, ty: ColTy, notnull: bool) -> Vec<ColDecl<'p>> { (cl.push(ColDecl { name: t.str(), ty, notnull }), cl).1 }

  #[rule(ConsList -> Cons)]
  fn cons_list0(c: Vec<TableCons<'p>>) -> Vec<TableCons<'p>> { c }
  #[rule(ConsList -> ConsList Comma Cons)]
  fn cons_list1(mut cl: Vec<TableCons<'p>>, _: Token, mut c: Vec<TableCons<'p>>) -> Vec<TableCons<'p>> { (cl.append(&mut c), cl).1 }

  #[rule(Cons -> Foreign Key LPar Id RPar References Id LPar Id RPar)]
  fn cons_foreign(_: Token, _: Token, _: Token, t: Token, _: Token, _: Token, table: Token, _: Token, col: Token, _: Token) -> Vec<TableCons<'p>> { vec![TableCons { name: t.str(), kind: TableConsKind::Foreign { table: table.str(), col: col.str() } }] }
  #[rule(Cons -> Primary Key LPar IdList RPar)]
  fn cons_primary(_: Token, _: Token, _: Token, il: Vec<&'p str>, _: Token) -> Vec<TableCons<'p>> { il.into_iter().map(|name| TableCons { name, kind: TableConsKind::Primary }).collect() }
  #[rule(Cons -> Unique LPar Id RPar)]
  fn cons_unique(_: Token, _: Token, t: Token, _: Token) -> Vec<TableCons<'p>> { vec![TableCons { name: t.str(), kind: TableConsKind::Unique }] }
  #[rule(Cons -> Check LPar Id RPar In LPar LitList RPar)]
  fn cons_check(_: Token, _: Token, t: Token, _: Token, _: Token, _: Token, ll: Vec<CLit<'p>>, _: Token) -> Vec<TableCons<'p>> { vec![TableCons { name: t.str(), kind: TableConsKind::Check(ll) }] }

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
  fn col_ref0(c: Token) -> ColRef<'p> { ColRef { table: None, col: c.str() } }
  #[rule(ColRef -> Id Dot Id)]
  fn col_ref1(t: Token, _: Token, c: Token) -> ColRef<'p> { ColRef { table: Some(t.str()), col: c.str() } }

  #[rule(WhereList -> WhereList And Cond)]
  fn where1(mut cl: Vec<Cond<'p>>, _: Token, c: Cond<'p>) -> Vec<Cond<'p>> { (cl.push(c), cl).1 }
  #[rule(WhereList -> Cond)]
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
  fn lit_int(&mut self, t: Token) -> CLit<'p> {
    let (s, line, col) = (t.str(), t.line, t.col);
    CLit::new(Lit::Number(s.parse::<i32>().unwrap_or_else(|_| (self.0.push(PE { line, col, kind: InvalidInt(s) }), 0).1) as f64))
  }
  #[rule(Lit -> FloatLit)]
  fn lit_float(&mut self, t: Token) -> CLit<'p> {
    let (s, line, col) = (t.str(), t.line, t.col);
    CLit::new(Lit::Number(s.parse::<f32>().unwrap_or_else(|_| (self.0.push(PE { line, col, kind: InvalidFloat(s) }), 0.0).1) as f64))
  }
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
  #[rule(BareTy -> VarChar)]
  fn bare_ty_var_char(_: Token) -> BareTy { VarChar }

  #[rule(ColTy -> BareTy LPar IntLit RPar)]
  fn col_ty(&mut self, ty: BareTy, _: Token, t: Token, _: Token) -> ColTy {
    let (s, line, col) = (t.str(), t.line, t.col);
    ColTy { size: s.parse().unwrap_or_else(|_| (self.0.push(PE { line, col, kind: TypeSizeTooLarge(s) }), 0).1), ty }
  }
  #[rule(ColTy -> Bool)]
  fn col_ty_bool(_: Token) -> ColTy { ColTy { size: 0, ty: Bool } }
  #[rule(ColTy -> Int)]
  fn col_ty_int(_: Token) -> ColTy { ColTy { size: 0, ty: Int } }
  #[rule(ColTy -> Float)]
  fn col_ty_float(_: Token) -> ColTy { ColTy { size: 0, ty: Float } }
  #[rule(ColTy -> Date)]
  fn col_ty_date(_: Token) -> ColTy { ColTy { size: 0, ty: Date } }
}