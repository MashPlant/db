use common::{*, Error::*};

bitflags::bitflags! {
  pub struct ColFlags: u32 {
    // PRIMARY implies NOTNULL, but doesn't imply UNIQUE
    // PRIMARY itself is only useful when their is multiple primary key, if it is a single primary key,
    // UNIQUE will be set, UNIQUE and NOTNULL will detect all errors
    const PRIMARY = 0b1;
    const NOTNULL = 0b10;
    const UNIQUE = 0b100;
  }
}

#[repr(C)]
pub struct ColInfo {
  pub ty: ColTy,
  // offset in a record
  pub off: u16,
  // index root page id, !0 for none
  pub index: u32,
  // index in DbPage::tables, !0 for none
  pub foreign_table: u8,
  // index in TablePage::cols, if foreign_table == !0, foreign_col is meaningless
  pub foreign_col: u8,
  pub flags: ColFlags,
  pub name_len: u8,
  pub name: [u8; MAX_COL_NAME as usize],
}

impl ColInfo {
  pub unsafe fn name<'a>(&self) -> &'a str {
    str_from_parts(self.name.as_ptr(), self.name_len as usize)
  }
}

#[repr(C)]
pub struct TablePage {
  // the prev and next in both TablePage and DataPage forms a circular linked list
  // initially table.prev = table.next = table page id, for an empty  linked list
  // they may be accessed by field ref or by [0] and [1]
  pub prev: u32,
  pub next: u32,
  // !0 for none
  pub first_free: u32,
  // there are at most 64G/16 records, so u32 is enough
  pub count: u32,
  // the size of a single slot, including null-bitset and data
  pub size: u16,
  // always equal to MAX_DATA_BYTE / size, store it just to avoid division
  pub cap: u16,
  pub col_num: u8,
  pub _rsv: [u8; 45],
  pub cols: [ColInfo; MAX_COL as usize],
}

pub const MAX_COL_NAME: u32 = 50;
pub const MAX_COL: u32 = 127;

impl TablePage {
  #[inline(always)]
  pub fn init(&mut self, id: u32, size: u16, col_num: u8) {
    (self.prev = id, self.next = id);  // self-circle to represent empty linked list
    self.count = 0;
    self.size = size;
    self.cap = MAX_DATA_BYTE as u16 / size;
    self.col_num = col_num;
  }

  pub unsafe fn names<'a>(&'a self) -> impl Iterator<Item=&'a str> + 'a {
    let col_num = self.col_num as usize;
    self.cols.iter().enumerate().filter_map(move |(i, ci)| if i < col_num { Some(ci.name()) } else { None })
  }

  #[inline(always)]
  pub unsafe fn get_ci<'a>(&mut self, col: &str) -> Result<&'a mut ColInfo> {
    match self.pr().names().enumerate().find(|n| n.1 == col) {
      Some((idx, _)) => Ok(self.pr().cols.get_unchecked_mut(idx)),
      None => return Err(NoSuchCol(col.into())),
    }
  }

  #[inline(always)]
  pub unsafe fn id_of(&self, col: &ColInfo) -> usize {
    (col as *const ColInfo).offset_from(self.cols.as_ptr()) as usize
  }
}