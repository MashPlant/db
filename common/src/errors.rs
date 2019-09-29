use crate::{MAGIC_LEN, ColTy, BareTy, LitTy, OwnedLit, AggOp};

#[derive(Debug)]
pub struct ParserError {
  pub line: u32,
  pub col: u32,
  pub kind: ParserErrorKind,
}

#[derive(Debug)]
pub enum ParserErrorKind {
  SyntaxError,
  UnrecognizedChar(char),
  TypeSizeTooLarge(Box<str>),
  InvalidInt(Box<str>),
  InvalidFloat(Box<str>),
}

#[derive(Debug)]
pub enum Error {
  ParserErrors(Box<[ParserError]>),
  InvalidSize(usize),
  InvalidMagic([u8; MAGIC_LEN]),
  NoDbInUse,
  TableExhausted,
  ColTooMany(usize),
  ColSizeTooBig(usize),
  TableNameTooLong(Box<str>),
  ColNameTooLong(Box<str>),
  DupTable(Box<str>),
  DupCol(Box<str>),
  ForeignKeyOnNonUnique(Box<str>),
  DupIndex(Box<str>),
  DropIndexOnUnique(Box<str>),
  NoSuchTable(Box<str>),
  NoSuchCol(Box<str>),
  NoSuchIndex(Box<str>),
  // there is no UpdateTableWithForeignLink, it will be rejected by UpdateWithIndex
  DropTableWithForeignLink(Box<str>),
  DeleteTableWithForeignLink(Box<str>),
  InvalidLike(regex::Error),
  InvalidLikeTy(BareTy),
  IncompatibleForeignTy { foreign: ColTy, own: ColTy },
  RecordTyMismatch { expect: BareTy, actual: BareTy },
  RecordLitTyMismatch { expect: BareTy, actual: LitTy },
  InsertLenMismatch { expect: u8, actual: usize },
  // these 2 can be used in both insert and update, so call them put
  PutStrTooLong { limit: u8, actual: usize },
  PutNullOnNotNull,
  InsertDupOnUniqueKey { col: Box<str>, val: OwnedLit },
  InsertNoExistOnForeignKey { col: Box<str>, val: OwnedLit },
  InsertDupCompositePrimaryKey,
  InsertNotInCheck { col: Box<str>, val: OwnedLit },
  InvalidDate { date: Box<str>, reason: chrono::ParseError },
  // below 2 not supported
  UpdateWithIndex(Box<str>),
  UpdateWithCheck(Box<str>),
  AmbiguousCol(Box<str>),
  DupPrimary(Box<str>),
  DupForeign(Box<str>),
  DupCheck(Box<str>),
  CheckNull(Box<str>),
  CheckTooLong(Box<str>, usize),
  InvalidAgg { col: ColTy, op: AggOp },
  CSV(csv::Error),
  IO(std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
  fn from(e: std::io::Error) -> Self { Error::IO(e) }
}

impl From<csv::Error> for Error {
  fn from(e: csv::Error) -> Self { Error::CSV(e) }
}