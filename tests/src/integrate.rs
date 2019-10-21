use typed_arena::Arena;

use driver::Eval;
use physics::*;
use common::{*, BareTy::*};

// format! input stmts to cover related code
macro_rules! ok { ($e: expr, $sql: expr) => { $e.exec_all($sql, &Arena::default(), |x| { let _ = format!("{:?}", x); }, |_| {}).unwrap(); }; }
macro_rules! err { ($e: expr, $sql: expr) => { $e.exec_all($sql, &Arena::default(), |x| { let _ = format!("{:?}", x); }, |_| {}).unwrap_err(); }; }

#[test]
#[ignore]
fn create() { ok!(Eval::default(), include_str!("../sql/build.sql")); }

fn select() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  err!(e, "select o_orderkey from ORDERS; -- error");
  err!(e, "select ORDER.O_ORDERKEY from ORDERS; -- error");

  ok!(e, "select O_ORDERKEY, O_ORDERSTATUS, O_TOTALPRICE from ORDERS;");
  ok!(e, "select * from ORDERS where O_ORDERKEY is not null;");
  ok!(e, "select * from ORDERS where O_ORDERDATE > '1996-10-06';");
  ok!(e, "select * from CUSTOMER where C_ADDRESS like 'IVhzIApeRb o_,c,E';");
  ok!(e, "select * from CUSTOMER where C_ADDRESS like 'XSTf4,NCwDVaWNe6tEgvwfmRch%';");

  ok!(e, "select * from ORDERS where O_CUSTKEY < 5; -- these select uses index");
  ok!(e, "select * from ORDERS where O_CUSTKEY <= 5;");
  ok!(e, "select * from ORDERS where O_CUSTKEY > 745;");
  ok!(e, "select * from ORDERS where O_CUSTKEY >= 745;");
  ok!(e, "select * from ORDERS where O_CUSTKEY = 567;");
  ok!(e, "select * from ORDERS where O_CUSTKEY = 0;");
  ok!(e, "select * from ORDERS where O_CUSTKEY = 751;");

  err!(e, "select O_ORDERKEY, avg(O_TOTALPRICE) from ORDERS; -- error, mixed select");
  ok!(e, "select avg(O_TOTALPRICE), min(O_TOTALPRICE), max(O_TOTALPRICE) from ORDERS where O_TOTALPRICE >= 100000;");

  ok!(e, "select * from ORDERS, CUSTOMER, NATION where O_CUSTKEY = C_CUSTKEY and C_NATIONKEY = N_NATIONKEY and N_NAME <> 'INDIA';");

  ok!(e, "create table test (name varchar(10));");
  ok!(e, r#"insert into test values ('''\n\r\t\');"#);
  err!(e, r#"insert into test values ('\n\n\n\n\n\n'); -- error, too long (\n is interpreted literally)"#);
  ok!(e, r#"select * from test where name like '%\';"#);
  ok!(e, r#"select * from test where name like '%\\'; -- the same as above"#);
  ok!(e, r#"insert into test values ('%%__\\''');"#);
  ok!(e, r#"select * from test where name like '\%\%\_\_\\\\''';"#);
  ok!(e, "insert into test values (null);");
  ok!(e, "select count(name) from test; -- 2");
  ok!(e, "drop table test;");

  ok!(e, "create table t1 (f float, s varchar(10)); create table t2 (s varchar(5), f float);");
  ok!(e, "insert into t1 values (1, '1'), (3, '3'), (5, '5'), (7, '7');  insert into t2 values ('2', 2), ('4', 4), ('6', 6), ('8', 8);");
  ok!(e, "select * from t1, t2 where t1.f < t2.f; select * from t1, t2 where t2.s > t1.s;");
  ok!(e, "select * from t2, t1 where t1.f < t2.f; select * from t2, t1 where t2.s > t1.s;");
  ok!(e, "drop table t1; drop table t2;");
}

fn insert() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  ok!(e, "create table test (i int, b bool default true, f float default 233, v varchar(10) default 'world', d date);");

  ok!(e, "insert into test values (19260817, false, 19260817.0, 'hello', '2019-10-01');");
  ok!(e, "select * from test where i = 19260817 and b = false and f = 19260817.0 and v = 'hello' and d = '2019-10-01';");
  ok!(e, "select * from test where i = f and b = b and f = i and v = v and d = d;");

  ok!(e, "create table test1 (i int, b bool, f float, v varchar(10), d date);");
  ok!(e, "insert into test1 values (19260817, false, 19260817.0, 'hello', '2019-10-01');");
  ok!(e, "select * from test, test1 where test.i = test1.f and test.b = test1.b and test.f = test1.i and test.v = test1.v and test.d = test1.d;");

  ok!(e, "insert into test (d, i) values ('2019-10-01', -233);");
  ok!(e, "insert into test values (666);");
  ok!(e, "select * from test;");
  err!(e, "insert into test (i, b, f, v) values (1, true, 1, '1', '2019-10-01'); -- error, too long");
  err!(e, "insert into test values (1, true, 1, '1', '2019-10-01', 1); -- error, too long");

  ok!(e, "drop table test;");
  ok!(e, "drop table test1;");
}

fn update() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  ok!(e, "update LINEITEM set L_LINENUMBER = -L_LINENUMBER where L_LINENUMBER > 4;");
  ok!(e, "update LINEITEM set L_LINENUMBER = -L_LINENUMBER where L_LINENUMBER < -4;");

  err!(e, "update CUSTOMER set C_CUSTKEY = -C_CUSTKEY; -- error, there are foreign link to customer");
  err!(e, "update LINEITEM set L_LINENUMBER = 0; -- error, dup primary key (one update will success)");

  ok!(e, "update LINEITEM set L_LINENUMBER = L_LINENUMBER + 1 - 2 * 3 / 4 % 5 - 1000000; -- note that / is fdiv, % is fmod");

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

  err!(e, "delete from ORDERS where O_ORDERKEY1 > 0; -- error");
  err!(e, "delete from ORDERS where order.O_ORDERKEY > 0; -- error");

  ok!(e, "select count(*) from LINEITEM;");
  ok!(e, "delete from LINEITEM where L_ORDERKEY > 15000;");
  ok!(e, "select count(*) from LINEITEM;");

  err!(e, "delete from CUSTOMER; -- error, there are foreign link to customer");
}

fn errors() {
  let mut e = Eval::default();
  err!(e, "^ -- error");
  err!(e, "; -- error");
  err!(e, "show database OrderDB; -- error");
  err!(e, "use OrderDB; -- error");
  ok!(e, "use orderDB;");
  err!(e, "create table CUSTOMER(id INT(10) NOT NULL); -- error, duplicate");
  err!(e, "create table t (id INT, id INT); -- error, duplicate");
  err!(e, "create table t (id INT(256) NOT NULL); -- error, u8 overflow");
  ok!(e, "create table t (id INT(255) NOT NULL);");
  err!(e, "insert into t value (2147483648); -- error, i32 overflow");
  err!(e, "insert into t values (null); -- error");
  err!(e, "create table t1 (id INT(255), CHECK (id IN ('F', 'M'))); -- error, check ty mismatch");
  ok!(e, "create table t1 (id DATE, CHECK (id IN ('2019-01-01')));");
  err!(e, "select id from t, t1; -- error, ambiguous col");
  err!(e, "drop table t2; -- error, no such table");
  ok!(e, "drop table t;");
  ok!(e, "drop table t1;");
}

#[test]
fn integrate() {
  create();
  errors();
  select();
  insert();
  update();
  delete();
  ok!(Eval::default(), include_str!("../sql/drop.sql"));
}