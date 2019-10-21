use std::{io, result, fmt};

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
  IndexNameTooLong(&'a str),
  DupTable(&'a str),
  DupCol(&'a str),
  DupIndex(&'a str),
  // add duplicate constraint on one col in create/alter table
  DupConstraint(&'a str),
  NoSuchTable(&'a str),
  NoSuchCol(&'a str),
  NoSuchIndex(&'a str),
  NoSuchForeign,
  ForeignOnNotUnique(&'a str),
  // this includes delete/drop table/update that actually affects a col with a foreign link
  ModifyColWithForeignLink { col: &'a str, val: CLit<'a> },
  InvalidDate { date: &'a str, reason: chrono::ParseError },
  InvalidLike { like: &'a str, reason: Box<regex::Error> },
  InvalidLikeTy(BareTy),
  InvalidLikeTy1(LitTy),
  // require them to be exactly the same (including BareTy and size, in order to search each other in index page)
  IncompatibleForeignTy { foreign: ColTy, own: ColTy },
  RecordTyMismatch { expect: BareTy, actual: BareTy },
  RecordLitTyMismatch { expect: BareTy, actual: LitTy },
  // e.g.: insert (1, 2) into (int)
  InsertTooLong { max: usize, actual: usize },
  // Put stands for Insert or Update
  PutStrTooLong { limit: u8, actual: usize },
  PutNullOnNotNull,
  PutDupOnUnique { col: &'a str, val: CLit<'a> },
  PutNonexistentForeign { col: &'a str, val: CLit<'a> },
  PutNotInCheck { col: &'a str, val: CLit<'a> },
  PutDupCompositePrimary,
  AmbiguousCol(&'a str),
  // check list always rejects null (because it is meaningless)
  CheckNull(&'a str),
  CheckTooLong(&'a str),
  InvalidAgg { col: ColTy, op: AggOp },
  // select agg col together with non-agg col
  MixedSelect,
  Div0,
  Mod0,
  IncompatibleBin { op: BinOp, ty: LitTy },
  IncompatibleCmp { op: CmpOp, l: LitTy, r: LitTy },
  IncompatibleLogic(LitTy),
  IO(io::Error),
}

// after modifying `self.0` columns, a `self.1` error occurs
pub struct ModifyError<'a>(pub u32, pub Error<'a>);

pub type Result<'a, T> = result::Result<T, Error<'a>>;
pub type ModifyResult<'a, T> = result::Result<T, ModifyError<'a>>;

impl From<io::Error> for Error<'_> { fn from(e: io::Error) -> Self { Error::IO(e) } }

impl From<io::Error> for ModifyError<'_> { fn from(e: io::Error) -> Self { Self(0, e.into()) } }

impl<'a> From<Error<'a>> for ModifyError<'a> { fn from(e: Error<'a>) -> Self { Self(0, e) } }

impl fmt::Debug for ModifyError<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}; {} column(s) affected", self.1, self.0)
  }
}