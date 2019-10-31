use typed_arena::Arena;

use driver::Eval;

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
  ok!(e, "select O_ORDERDATE, O_ORDERPRIORITY from ORDERS where O_ORDERKEY is not null;");
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

  ok!(e, "create table t1 (f float, d date, s char(10)); create table t2 (s char(5), f float, d date);");
  ok!(e, "insert into t1 values (1, '2019-01-01', '1'), (3, '2019-01-03', '3'), (5, '2019-01-05', '5'), (7, '2019-01-07', '7');");
  ok!(e, "insert into t2 values ('2', 2, '2019-01-02'), ('4', 4, '2019-01-04'), ('6', 6, '2019-01-06'), ('8', 8, '2019-01-08');");
  ok!(e, "select * from t1, t2 where t1.f < t2.f; select * from t1, t2 where t2.s > t1.s; select * from t1, t2 where t2.d > t1.d;");
  ok!(e, "select * from t2, t1 where t1.f < t2.f; select * from t2, t1 where t2.s > t1.s; select * from t2, t1 where t2.d > t1.d;");
  ok!(e, "select * from t1, t2 where t1.f <> t2.f and t1.s <> t2.s; -- equivalent to no condition");
  ok!(e, "drop table t1; drop table t2;");

  ok!(e, "create table t1 (f float, d date, s varchar(10)); create table t2 (s varchar(5), f float, d date); -- like above, but use varchar, some optimization may fail");
  ok!(e, "insert into t1 values (1, '2019-01-01', '1'), (3, '2019-01-03', '3'), (5, '2019-01-05', '5'), (7, '2019-01-07', '7');");
  ok!(e, "insert into t2 values ('2', 2, '2019-01-02'), ('4', 4, '2019-01-04'), ('6', 6, '2019-01-06'), ('8', 8, '2019-01-08');");
  ok!(e, "select * from t1, t2 where t1.f < t2.f; select * from t1, t2 where t2.s > t1.s; select * from t1, t2 where t2.d > t1.d;");
  ok!(e, "select * from t2, t1 where t1.f < t2.f; select * from t2, t1 where t2.s > t1.s; select * from t2, t1 where t2.d > t1.d;");
  ok!(e, "select * from t1, t2 where t1.f <> t2.f and t1.s <> t2.s; -- equivalent to no condition");
  ok!(e, "drop table t1; drop table t2;");

  ok!(e, "create table test (c char(10), v1 varchar(20), v2 varchar(30));");
  ok!(e, "insert into test values ('hello', 'hello', 'world');");
  ok!(e, "insert into test values ('world', 'hello', 'hello');");
  ok!(e, "select * from test where c = v1 and v1 = c;");
  ok!(e, "select * from test where v1 = v2;");
  ok!(e, "drop table test;");
}

fn insert() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  ok!(e, "create table test (i int, b bool default true, f float default 233, v char(10) default 'world', d date, check (v in ('hello', 'world')));");
  ok!(e, "desc test;");

  err!(e, "insert into test (v) values ('foo'); -- error, not in check");
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

  ok!(e, "create table test(i int, v char(10), b bool, primary key (v, b), unique(i));");
  ok!(e, "insert into test values (1, 'hello', true);");
  ok!(e, "update test set b = i < 0 and v like 'he_lo';");
  ok!(e, "update test set b = i < 0 or v like 'he_lo';");
  ok!(e, "update test set b = i is not null and v is not null; -- now the only key in test is (1, 'hello', true)");
  ok!(e, "insert into test values (2, 'hello', false);");
  err!(e, "update test set i = 1 where i = 2; -- error, dup i");
  err!(e, "update test set b = true where i = 2; -- error, dup composite primary key");
  ok!(e, "drop table test;");

  ok!(e, "create table test (v1 varchar(2) not null, v2 varchar(2));");
  ok!(e, "insert into test values ('v1', 'v2');");

  err!(e, "update test set v1 = 'long', v2 = 'v2'; -- error, and `lit2varchar` should never be called, belows are the same");
  err!(e, "update test set v1 = 'v1', v2 = 'long'; -- error");
  err!(e, "update test set v1 = 'v1', v2 = 233; -- error");
  err!(e, "update test set v1 = 233, v2 = 'v2'; -- error");
  err!(e, "update test set v1 = null, v2 = 'long'; -- error, and `free_varchar` should never be called");
  ok!(e, "update test set v1 = 'v2', v2 = null;");
  ok!(e, "select * from test;");
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

fn alter() {
  let mut e = Eval::default();
  ok!(e, "use orderDB;");

  ok!(e, "select sum(C_CUSTKEY), sum(C_ACCTBAL) from CUSTOMER;");
  ok!(e, "alter table CUSTOMER drop C_ADDRESS;");
  ok!(e, "alter table CUSTOMER add foo char(10) not null default 'foo';");
  ok!(e, "select sum(C_CUSTKEY), sum(C_ACCTBAL) from CUSTOMER;");

  ok!(e, "create table test (i int, v varchar(10));");

  ok!(e, "alter table test add b bool;");

  ok!(e, "insert into test values (1, 'hello', true);");
  err!(e, "alter table test add f bool not null; -- error, f will be null");
  ok!(e, "alter table test add f float default 233;");
  ok!(e, "insert into test values (0, 'world', false);");
  ok!(e, "select * from test;");

  ok!(e, "alter table test drop b; alter table test drop v; alter table test drop f;");
  err!(e, "alter table test drop i; -- error, col num will be 0");

  ok!(e, "drop table test;");

  ok!(e, "create table test1 (a int, b int, primary key(a));");
  ok!(e, "create table test2 (v1 varchar(10), v2 varchar(10), f_a int, f_b int, foreign key(f_a) references test1(a));");
  err!(e, "alter table test1 add primary key (a, a); -- error, dup col");
  err!(e, "alter table test1 add primary key (a); -- error, dup constraint");
  err!(e, "alter table test1 drop a; -- error, there is foreign link to a");
  err!(e, "alter table test1 add primary key (b); -- error, a will not be unique");
  err!(e, "alter table test1 drop primary key (a); -- error, a will not be unique");
  ok!(e, "drop table test2;");
  ok!(e, "drop table test1;");

  ok!(e, "create table test (a int, b int);");
  ok!(e, "insert into test values (1, 1), (1, 2);");
  ok!(e, "alter table test add primary key(a, b);");
  err!(e, "alter table test drop b; -- error, a will be duplicate");
  err!(e, "alter table test drop primary key (b); -- error, a is duplicate");
  ok!(e, "alter table test drop primary key (a, b);");
  err!(e, "alter table test add primary key (a); -- error, a is duplicate");
  ok!(e, "alter table test add c int;");
  err!(e, "alter table test drop primary key(c); -- error, c is not primary");
  err!(e, "alter table test add primary key(c); -- error, c is null");
  ok!(e, "drop table test;");
}

fn errors() {
  let mut e = Eval::default();
  err!(e, "^ -- error");
  err!(e, "; -- error");
  err!(e, "show database OrderDB; -- error");
  err!(e, "use OrderDB; -- error");
  ok!(e, "use orderDB;");
  err!(e, "create table CUSTOMER(id int(10) not null); -- error, duplicate");
  err!(e, "create table t (id int, id int); -- error, duplicate");
  err!(e, "create table t (id int(256) not null); -- error, u8 overflow");
  ok!(e, "create table t (id int(255) not null);");
  err!(e, "insert into t value (2147483648); -- error, i32 overflow");
  err!(e, "insert into t values (null); -- error");
  err!(e, "create table t1 (id int(255), CHECK (id IN ('F', 'M'))); -- error, check ty mismatch");
  ok!(e, "create table t1 (id DATE, CHECK (id IN ('2019-01-01')));");
  err!(e, "select id from t, t1; -- error, ambiguous col");
  err!(e, "drop table t2; -- error, no such table");
  ok!(e, "drop table t;");
  ok!(e, "drop table t1;");

  err!(e, "create table t (v varchar(10), unique(v)); -- error, unsupported varchar op");
  err!(e, "create table t (v varchar(10), primary key (v)); -- error");
  err!(e, "create table t (v varchar(10) default ''); -- error");
  err!(e, "create table t (v varchar(10), check (v in (''))); -- error");
  ok!(e, "create table t (v varchar(10));");
  err!(e, "alter table t add index test_v_idx on(v); -- error");
  err!(e, "alter table t add primary key (v); -- error");
  err!(e, "create table t1 (v varchar(10), foreign key (v) references t(v)); -- error");
  ok!(e, "create table t1 (v varchar(10));");
  err!(e, "alter table t1 add foreign key (v) references t(v); -- error");
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
  alter();
  ok!(Eval::default(), include_str!("../sql/drop.sql"));
}