use driver::Eval;

const CREATE: &[u8] = include_bytes!("../sql/create.sql");
const DROP: &[u8] = include_bytes!("../sql/drop.sql");
const CUSTOMER: &[u8] = include_bytes!("../sql/customer.sql");
const BOOK: &[u8] = include_bytes!("../sql/book.sql");
const WEBSITE: &[u8] = include_bytes!("../sql/website.sql");
const PRICE: &[u8] = include_bytes!("../sql/price.sql");
const ORDERS: &[u8] = include_bytes!("../sql/orders.sql");


fn exec_all(code: &[u8], e: Option<Eval>) -> Eval {
  use syntax::*;
  let mut e = e.unwrap_or_else(Eval::new);
  for s in &Parser.parse(&mut Lexer::new(code)).unwrap() {
    let res = e.exec(s);
    res.unwrap();
  }
  e
}


#[test]
fn test() {
  use physics::*;
  use common::{*, BareTy::*};

  let mut e = exec_all(CREATE, None);
  unsafe {
    let db = e.db.as_mut().unwrap();
    let dp = db.get_page::<DbPage>(0);
    assert_eq!(dp.table_num, 5);
    {
      let t = db.get_tp("customer").unwrap();
      assert_eq!(t.col_num, 3);
      let c = &t.cols[0];
      assert_eq!(c.ty, ColTy { size: 10, ty: Int });
      assert_ne!(c.index, !0);
      assert_eq!(c.foreign_table, !0);
      assert_eq!(c.flags, ColFlags::PRIMARY | ColFlags::NOTNULL | ColFlags::UNIQUE);
      assert_eq!(c.name(), "id");
      let c = &t.cols[1];
      assert_eq!(c.ty, ColTy { size: 25, ty: VarChar });
      assert_eq!(c.index, !0);
      assert_eq!(c.foreign_table, !0);
      assert_eq!(c.flags, ColFlags::NOTNULL);
      assert_eq!(c.name(), "name");
      let c = &t.cols[2];
      assert_eq!(c.ty, ColTy { size: 1, ty: VarChar });
      assert_eq!(c.index, !0);
      assert_eq!(c.foreign_table, !0);
      assert_eq!(c.flags, ColFlags::NOTNULL);
      assert_eq!(c.name(), "gender");
    }
    {
      let t = db.get_tp("price").unwrap();
      assert_eq!(t.col_num, 3);
      let c = &t.cols[0];
      assert_eq!(c.ty, ColTy { size: 10, ty: Int });
      assert_eq!(c.index, !0);
      assert_eq!(c.foreign_table, 2); // website
      assert_eq!(c.foreign_col, 0); // website(id)
      assert_eq!(c.flags, ColFlags::PRIMARY | ColFlags::NOTNULL);
      assert_eq!(c.name(), "website_id");
      let c = &t.cols[1];
      assert_eq!(c.ty, ColTy { size: 10, ty: Int });
      assert_eq!(c.index, !0);
      assert_eq!(c.foreign_table, 1); // book
      assert_eq!(c.foreign_col, 0); // book(id)
      assert_eq!(c.flags, ColFlags::PRIMARY | ColFlags::NOTNULL);
      assert_eq!(c.name(), "book_id");
      let c = &t.cols[2];
      assert_eq!(c.ty, ColTy { size: 0, ty: Float });
      assert_eq!(c.index, !0);
      assert_eq!(c.foreign_table, !0);
      assert_eq!(c.flags, ColFlags::NOTNULL);
      assert_eq!(c.name(), "price");
    }
  }

  let e = exec_all(CUSTOMER, Some(e));
  let e = exec_all(BOOK, Some(e));
  let e = exec_all(WEBSITE, Some(e));
  let e = exec_all(PRICE, Some(e));
  let _ = exec_all(ORDERS, Some(e));

  exec_all(DROP, None);
}