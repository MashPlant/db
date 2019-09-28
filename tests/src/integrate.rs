use driver::Eval;

const CREATE: &str = include_str!("../sql/create.sql");
const DROP: &str = include_str!("../sql/drop.sql");
const CUSTOMER: &str = include_str!("../sql/customer.sql");
const BOOK: &str = include_str!("../sql/book.sql");
const WEBSITE: &str = include_str!("../sql/website.sql");
const PRICE: &str = include_str!("../sql/price.sql");
const ORDERS: &str = include_str!("../sql/orders.sql");

#[test]
fn test() {
  use physics::*;
  use common::{*, BareTy::*};

  let mut e = Eval::default();
  e.exec_all_check(CREATE);
  unsafe {
    let db = e.db.as_mut().unwrap();
    let dp = db.get_page::<DbPage>(0);
    assert_eq!(dp.table_num, 5);
    {
      let t = db.get_tp("customer").unwrap().1;
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
      let t = db.get_tp("price").unwrap().1;
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

  e.exec_all_check(CUSTOMER);
  e.exec_all_check(BOOK);
  e.exec_all_check(WEBSITE);
  e.exec_all_check(PRICE);
  e.exec_all_check(ORDERS);

//  exec_all(DROP, None);
}