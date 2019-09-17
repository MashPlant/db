use std::{fmt, io::Error as IOError, error};

use crate::{MAGIC_LEN, ColTy, BareTy, LitTy, OwnedLit};

#[derive(Debug)]
pub enum Error {
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
  CreateIndexOnNonEmpty(Box<str>),
  DropIndexOnUnique(Box<str>),
  NoSuchTable(Box<str>),
  NoSuchCol(Box<str>),
  NoSuchIndex(Box<str>),
  DropTableWithForeignLink(Box<str>),
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
  // CmpOnNull if lit is null (if data in db is null, != returns true, all others returns false)
  CmpOnNull,
  InvalidDate { date: Box<str>, reason: chrono::ParseError },
  // this is not supported
  UpdateWithIndex(Box<str>),
  AmbiguousCol(Box<str>),
  IO(IOError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<IOError> for Error {
  fn from(e: IOError) -> Self { Error::IO(e) }
}

impl error::Error for Error {}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
    write!(f, "{:?}", self)
  }
}