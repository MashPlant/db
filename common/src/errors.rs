use crate::{MAGIC_LEN, ColTy, BareTy, LitTy, Lit, AggOp};

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
  // there is no UpdateTableWithForeignLink, it will be rejected by UpdateWithIndex
  DropTableWithForeignLink(&'a str),
  DeleteTableWithForeignLink(&'a str),
  InvalidDate { date: &'a str, reason: chrono::ParseError },
  InvalidLike { like: &'a str, reason: regex::Error },
  InvalidLikeTy(BareTy),
  IncompatibleForeignTy { foreign: ColTy, own: ColTy },
  RecordTyMismatch { expect: BareTy, actual: BareTy },
  RecordLitTyMismatch { expect: BareTy, actual: LitTy },
  InsertLenMismatch { expect: u8, actual: usize },
  // these 2 can be used in both insert and update, so call them put
  PutStrTooLong { limit: u8, actual: usize },
  PutNullOnNotNull,
  InsertDupOnUniqueKey { col: &'a str, val: Lit<'a> },
  InsertNonexistentForeignKey { col: &'a str, val: Lit<'a> },
  InsertDupCompositePrimaryKey,
  InsertNotInCheck { col: &'a str, val: Lit<'a> },
  // below 2 not supported
  UpdateWithIndex(&'a str),
  UpdateWithCheck(&'a str),
  AmbiguousCol(&'a str),
  DupPrimary(&'a str),
  DupForeign(&'a str),
  DupCheck(&'a str),
  CheckNull(&'a str),
  CheckTooLong(&'a str, usize),
  InvalidAgg { col: ColTy, op: AggOp },
  // select agg col together with non-agg col
  MixedSelect,
  CSV(csv::Error),
  IO(std::io::Error),
}

pub type Result<'a, T> = std::result::Result<T, Error<'a>>;

impl From<std::io::Error> for Error<'_> {
  fn from(e: std::io::Error) -> Self { Error::IO(e) }
}

impl From<csv::Error> for Error<'_> {
  fn from(e: csv::Error) -> Self { Error::CSV(e) }
}