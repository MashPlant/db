use driver::Eval;
use physics::*;
use common::{*, BareTy::*};

const CREATE: &str = include_str!("../sql/create.sql");
const DROP: &str = include_str!("../sql/drop.sql");
const CUSTOMER: &str = include_str!("../sql/customer.sql");
const BOOK: &str = include_str!("../sql/book.sql");
const WEBSITE: &str = include_str!("../sql/website.sql");
const PRICE: &str = include_str!("../sql/price.sql");
const ORDERS: &str = include_str!("../sql/orders.sql");

#[test]
#[ignore]
fn create() {
  let mut e = Eval::default();
  e.exec_all(CREATE, |_| {}, |_| {}).unwrap();
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

  e.exec_all(CUSTOMER, |_| {}, |_| {}).unwrap();
  e.exec_all(BOOK, |_| {}, |_| {}).unwrap();
  e.exec_all(WEBSITE, |_| {}, |_| {}).unwrap();
  e.exec_all(PRICE, |_| {}, |_| {}).unwrap();
  e.exec_all(ORDERS, |_| {}, |_| {}).unwrap();
}

#[test]
#[ignore]
fn select() {
  let mut e = Eval::default();
  e.exec_all("use orderDB;", |_| {}, |_| {}).unwrap();
  e.exec_all("select * from orders;", |_| {}, |_| {}).unwrap();
  e.exec_all("select * from orders where id is not null;", |_| {}, |_| {}).unwrap();
  e.exec_all("select * from orders where date0 > '2017-09-26';", |_| {}, |_| {}).unwrap();
  let _ = e.exec_all("drop index orders (customer_id);", |_| {}, |_| {}); // maybe fail because index doesn't exist yet, but doesn't matter
  e.exec_all("select * from orders where customer_id=306967;", |_| {}, |_| {}).unwrap();
  e.exec_all("create index orders (customer_id);", |_| {}, |_| {}).unwrap();
  e.exec_all("select * from orders where customer_id=306967;", |_| {}, |_| {}).unwrap();
  e.exec_all("select * from customer where name like 'chad ca_ello';", |_| {}, |_| {}).unwrap();
  e.exec_all("select * from customer where name like 'fausto vanno%';", |_| {}, |_| {}).unwrap();
  assert!(e.exec_all("select website_id, avg(price) from price;", |_| {}, |_| {}).is_err());
  e.exec_all("select avg(price), min(price), max(price), count(price), count(*) from price where price>=60;", |_| {}, |_| {}).unwrap();
  e.exec_all("select *
from orders, customer, website
where website.id=orders.website_id and customer.id=orders.customer_id and orders.quantity > 5;", |_| {}, |_| {}).unwrap();
}

#[test]
#[ignore]
fn errors() {
  let mut e = Eval::default();
  assert!(e.exec_all("^", |_| {}, |_| {}).is_err());
  assert!(e.exec_all(";", |_| {}, |_| {}).is_err());
  assert!(e.exec_all("use OrderDB; -- typo", |_| {}, |_| {}).is_err());
  e.exec_all("use orderDB;", |_| {}, |_| {}).unwrap();
  assert!(e.exec_all("CREATE TABLE customer( -- duplicate
    id INT(10) NOT NULL
);", |_| {}, |_| {}).is_err());
  assert!(e.exec_all("CREATE TABLE t(
    id INT(256) NOT NULL -- u8 overflow
);", |_| {}, |_| {}).is_err());
  e.exec_all("CREATE TABLE t(
    id INT(255) NOT NULL
);", |_| {}, |_| {}).unwrap();
  assert!(e.exec_all("insert into t values (2147483648); -- i32 overflow", |_| {}, |_| {}).is_err());
}

#[test]
fn integrate() {
  create();
  errors();
  select();
  Eval::default().exec_all(DROP, |_| {}, |_| {}).unwrap();
}