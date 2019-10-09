use crate::{MAGIC_LEN, ColTy, BareTy, LitTy, CLit, AggOp, BinOp, CmpOp};

#[derive(Debug)]
pub struct ParserError<'a> {
  pub line: u32,
  pub col: u32,
  pub kind: ParserErrorKind<'a>,
}

#[derive(Debug)]
pub enum ParserErrorKind<'a> {
  SyntaxError,
  UnrecognizedChar(char),
  TypeSizeTooLarge(&'a str),
  InvalidInt(&'a str),
  InvalidFloat(&'a str),
}

#[derive(Debug)]
pub enum Error<'a> {
  ParserErrors(Box<[ParserError<'a>]>),
  InvalidSize(usize),
  InvalidMagic([u8; MAGIC_LEN]),
  NoDbInUse,
  TableExhausted,
  ColTooMany(usize),
  ColSizeTooBig(usize),
  TableNameTooLong(&'a str),
  ColNameTooLong(&'a str),
  DupTable(&'a str),
  DupCol(&'a str),
  ForeignKeyOnNonUnique(&'a str),
  DupIndex(&'a str),
  DropIndexOnUnique(&'a str),
  NoSuchTable(&'a str),
  NoSuchCol(&'a str),
  NoSuchIndex(&'a str),
  // this includes delete from/drop a table that have a col with a foreign link
  // and update a col with a foreign link (strictly speaking, in this case its name should be "AlterCol")
  AlterTableWithForeignLink(&'a str),
  InvalidDate { date: &'a str, reason: chrono::ParseError },
  InvalidLike { like: &'a str, reason: Box<regex::Error> },
  InvalidLikeTy(BareTy),
  InvalidLikeTy1(LitTy),
  IncompatibleForeignTy { foreign: ColTy, own: ColTy },
  RecordTyMismatch { expect: BareTy, actual: BareTy },
  RecordLitTyMismatch { expect: BareTy, actual: LitTy },
  InsertLenMismatch { expect: u8, actual: usize },
  // Put stands for Insert or Update
  PutStrTooLong { limit: u8, actual: usize },
  PutNullOnNotNull,
  PutDupOnUniqueKey { col: &'a str, val: CLit<'a> },
  PutNonexistentForeignKey { col: &'a str, val: CLit<'a> },
  PutNotInCheck { col: &'a str, val: CLit<'a> },
  PutDupCompositePrimaryKey,
  AmbiguousCol(&'a str),
  // below 4 are duplicate constraint on one col in creating
  DupPrimary(&'a str),
  DupForeign(&'a str),
  DupUnique(&'a str),
  DupCheck(&'a str),
  CheckNull(&'a str),
  CheckTooLong(&'a str, usize),
  InvalidAgg { col: ColTy, op: AggOp },
  // select agg col together with non-agg col
  MixedSelect,
  Div0,
  Mod0,
  IncompatibleBin { op: BinOp, ty: LitTy },
  IncompatibleCmp { op: CmpOp, l: LitTy, r: LitTy },
  IncompatibleLogic(LitTy),
  IO(std::io::Error),
}

pub type Result<'a, T> = std::result::Result<T, Error<'a>>;

impl From<std::io::Error> for Error<'_> {
  fn from(e: std::io::Error) -> Self { Error::IO(e) }
}