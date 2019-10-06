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

// format! input stmts to cover related code
macro_rules! ok { ($e: expr, $sql: expr) => { $e.exec_all($sql, |x| { let _ = format!("{:?}", x); }, |_| {}).unwrap(); }; }
macro_rules! err { ($e: expr, $sql: expr) => { $e.exec_all($sql, |x| { let _ = format!("{:?}", x); }, |_| {}).unwrap_err(); }; }

#[test]
#[ignore]
fn create() {
  let mut e = Eval::default();
  ok!(e, CREATE);
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
  ok!(e, CUSTOMER);
  ok!(e, BOOK);
  ok!(e, WEBSITE);
  ok!(e, PRICE);
  ok!(e, ORDERS);
}

#[test]
#[ignore]
fn select() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");
  ok!(e, "select * from orders;");
  ok!(e, "select * from orders where id is not null;");
  ok!(e, "select * from orders where date0 > '2017-09-26';");
  let _ = e.exec_all("drop index orders (customer_id);", |_| {}, |_| {});
  // maybe fail because index doesn't exist yet, but doesn't matter
  ok!(e, "select * from orders where customer_id=306967;");
  ok!(e, "create index orders (customer_id);");
  ok!(e, "select * from orders where customer_id=306967;");
  ok!(e, "select * from customer where name like 'chad ca_ello';");
  ok!(e, "select * from customer where name like 'fausto vanno%';");
  assert!(e.exec_all("select website_id, avg(price) from price;", |_| {}, |_| {}).is_err());
  ok!(e, "select avg(price), min(price), max(price), count(price), count(*) from price where price>=60;");
  ok!(e, "select *
from orders, customer, website
where website.id=orders.website_id and customer.id=orders.customer_id and orders.quantity > 5;");
}

#[test]
#[ignore]
fn errors() {
  let mut e = Eval::default();
  err!(e, "^");
  err!(e, ";");
  err!(e, "SHOW DATABASE OrderDB; -- typo");
  err!(e, "use OrderDB; -- typo");
  ok!(e, "use orderDB; -- ok");
  err!(e, "CREATE TABLE t (id INT, id INT); -- duplicate -- duplicate");
  err!(e, "CREATE TABLE customer(id INT(10) NOT NULL); -- duplicate");
  err!(e, "CREATE TABLE t (id INT(256) NOT NULL); -- u8 overflow");
  ok!(e, "CREATE TABLE t (id INT(255) NOT NULL); -- ok");
  err!(e, "insert into t value (2147483648); -- i32 overflow");
  err!(e, "CREATE TABLE t1 (id INT(255), CHECK (id) IN ('F', 'M')); -- invalid check");
  ok!(e, "CREATE TABLE t1 (id DATE, CHECK (id) IN ('2019-01-01')); -- ok");
}

#[test]
#[ignore]
fn insert() {
  let mut e = Eval::default();
  ok!(e, "use orderDB; -- ok");
  err!(e, "insert into orders values (1,1001,300001,200001,'2014-09-31',5); -- illegal date");
  err!(e, "insert into orders values (1,1000,300001,200001,'2014-09-30',5); -- no such website");
  err!(e, "insert into orders values (1,1001,3000000,200001,'2014-09-30',5); -- no such customer");
  ok!(e, "insert into orders values (1,1001,300001,200001,'2014-09-30',5); -- ok");
  err!(e, "insert into orders values (1,1001,300001,200001,'2014-09-30',5); -- duplicate id");
  err!(e, "insert into customer values (1, 'name', 'x'); -- not in check list");
  ok!(e, "delete from orders where id = 1; -- ok, remove the previously inserted value");
  err!(e, "insert into price values (1002,249932,9999); -- dup composite primary key");
  ok!(e, "insert into price values (1003,249932,9999); -- ok");
  ok!(e, "delete from price where price = 9999; -- ok, remove the previously inserted value");
}

#[test]
fn integrate() {
  crate::index::test_index();
  create();
  errors();
  select();
  insert();
  Eval::default().exec_all(DROP, |_| {}, |_| {}).unwrap();
}