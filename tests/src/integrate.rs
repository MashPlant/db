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
    let dp = db.dp();
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
      assert_eq!(c.foreign_table, db.get_tp("website").unwrap().0);
      assert_eq!(c.foreign_col, 0); // website(id)
      assert_eq!(c.flags, ColFlags::PRIMARY | ColFlags::NOTNULL);
      assert_eq!(c.name(), "website_id");
      let c = &t.cols[1];
      assert_eq!(c.ty, ColTy { size: 10, ty: Int });
      assert_eq!(c.index, !0);
      assert_eq!(c.foreign_table, db.get_tp("book").unwrap().0);
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

fn select() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  err!(e, "select id1 from orders; -- error");
  err!(e, "select order.id from orders; -- error");
  err!(e, "select id from orders, customer; -- error, ambiguous col");

  ok!(e, "select * from orders;");
  ok!(e, "select * from orders where id is not null;");
  ok!(e, "select * from orders where date0 > '2017-09-26';");
  ok!(e, "select * from customer where name like 'CHAD CA_ELLO';");
  ok!(e, "select * from customer where name like 'FAUSTO VANNO%';");

  ok!(e, "create index orders (customer_id);");
  ok!(e, "select * from orders where customer_id < 300002;");
  ok!(e, "select * from orders where customer_id <= 300002;");
  ok!(e, "select * from orders where customer_id > 306999;");
  ok!(e, "select * from orders where customer_id >= 306999;");
  ok!(e, "select * from orders where customer_id = 306967;");
  ok!(e, "drop index orders (customer_id);");

  err!(e, "select website_id, avg(price) from price; -- error, mixed select");
  ok!(e, "select avg(price), min(price), max(price) from price where price >= 60;");

  ok!(e, "select * from orders, customer, website where website.id=orders.website_id and customer.id=orders.customer_id and orders.quantity > 5;");

  ok!(e, "create table test (name varchar(10));");
  ok!(e, r#"insert into test values ('''\n\r\t\');"#);
  err!(e, r#"insert into test values ('\n\n\n\n\n\n'); -- error, too long (\n is interpreted literally)"#);
  ok!(e, r#"select * from test where name like '%\';"#);
  ok!(e, r#"select * from test where name like '%\\'; -- the same as above"#);
  ok!(e, r#"insert into test values ('%%__\\''');"#);
  ok!(e, r#"select * from test where name like '\%\%\_\_\\\\''';"#);
  ok!(e, "insert into test values (null);");
  ok!(e, "select count(name) from test; -- 2");
  ok!(e, r#"drop table test;"#);
}

fn insert() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  err!(e, "insert into orders values (1, 1001, 300001, 200001, '2014-09-30', 5, 'more'); -- error");
  err!(e, "insert into orders values (1, 1001, 300001, 200001, 'less'); -- error");

  err!(e, "insert into orders values (1, 1001, 300001, 200001, '2014-09-31', 5); -- error, illegal date");
  err!(e, "insert into orders values (1, 1000, 300001, 200001, '2014-09-30', 5); -- error, no such website");
  err!(e, "insert into orders values (1, 1001, 3000000, 200001, '2014-09-30', 5); -- error, no such customer");
  ok!(e, "insert into orders values (1, 1001, 300001, 200001, '2014-09-30', 5);");
  err!(e, "insert into orders values (1, 1001, 300001, 200001, '2014-09-30', 5); -- error, duplicate id");
  err!(e, "insert into customer values (1, 'name', 'x'); -- error, not in check list");
  ok!(e, "delete from orders where id = 1;");
  err!(e, "insert into price values (1002, 249932, 9999); -- error, dup composite primary key");
  ok!(e, "insert into price values (1003, 249932, 9999);");
  ok!(e, "delete from price where price = 9999;");

  ok!(e, "create table test (i int, b bool, f float, v varchar(10), d date); -- have all data types");
  ok!(e, "insert into test values (19260817, false, 19260817.0, 'hello', '2019-10-01');");
  ok!(e, "select * from test where i = 19260817 and b = false and f = 19260817.0 and v = 'hello' and d = '2019-10-01';");
  ok!(e, "select * from test where i = f and b = b and f = i and v = v and d = d;");
  ok!(e, "create table test1 (i int, b bool, f float, v varchar(10), d date);");
  ok!(e, "insert into test1 values (19260817, false, 19260817.0, 'hello', '2019-10-01');");
  ok!(e, "select * from test, test1 where test.i = test1.f and test.b = test1.b and test.f = test1.i and test.v = test1.v and test.d = test1.d;");
  ok!(e, "drop table test;");
  ok!(e, "drop table test1;");
}

fn update() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  err!(e, "update orders set id1 = 1; -- error");
  err!(e, "update orders set id = 1 where id1 = 1; -- error");
  err!(e, "update orders set id = 1 where order.id = 1; -- error");

  ok!(e, "update orders set id = -id where id > 150000;");
  ok!(e, "update orders set id = -id where id < -150000;");

  err!(e, "update customer set id = -id; -- error, there are foreign link to customer");
  err!(e, "update orders set id = 0; -- error, dup primary key (one update will success)");
  err!(e, "update orders set id = book_id + website_id / (customer_id - customer_id); -- error, div 0 gives null, id is notnull");

  ok!(e, "update orders set id = id + 1 - 2 * 3 / 4 % 5; -- note that / is fdiv, % is fmod");

  ok!(e, "create table test(i int, v varchar(10), b bool, primary key (v, b), unique(i));");
  ok!(e, "insert into test values (1, 'hello', true);");
  ok!(e, "update test set b = i < 0 and v like 'he_lo';");
  ok!(e, "update test set b = i < 0 or v like 'he_lo';");
  ok!(e, "update test set b = i is not null and v is not null; -- now the only key in test is (1, 'hello', true)");
  ok!(e, "insert into test values (2, 'hello', false);");
  err!(e, "update test set i = 1 where i = 2; -- error, dup i");
  err!(e, "update test set b = true where i = 2; -- error, dup composite primary key");
  ok!(e, "drop table test;");
}

fn delete() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  err!(e, "delete from orders where id1 > 0; -- error");
  err!(e, "delete from orders where order.id > 0; -- error");

  ok!(e, "select count(*) from orders;");
  ok!(e, "delete from orders where id > 150000;");
  ok!(e, "select count(*) from orders;");

  err!(e, "delete from customer; -- error, there are foreign link to customer");
}

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
  err!(e, "insert into t values (null);");
  err!(e, "CREATE TABLE t1 (id INT(255), CHECK (id) IN ('F', 'M')); -- invalid check");
  ok!(e, "CREATE TABLE t1 (id DATE, CHECK (id) IN ('2019-01-01')); -- ok");
}

#[test]
fn integrate() {
  create();
  errors();
  select();
  insert();
  update();
  delete();
  ok!(Eval::default(), DROP);
}