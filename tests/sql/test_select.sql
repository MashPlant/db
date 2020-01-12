use orderDB;

select o_orderkey from ORDERS; -- error
select ORDER.O_ORDERKEY from ORDERS; -- error

select O_ORDERKEY, O_ORDERSTATUS, O_TOTALPRICE from ORDERS;
select O_ORDERDATE, O_ORDERPRIORITY from ORDERS where O_ORDERKEY is not null;
select * from ORDERS where O_ORDERDATE > '1996-10-06';
select * from CUSTOMER where C_ADDRESS like 'IVhzIApeRb o_,c,E';
select * from CUSTOMER where C_ADDRESS like 'XSTf4,NCwDVaWNe6tEgvwfmRch%';

select * from ORDERS where O_CUSTKEY < 5; -- these select uses index
select * from ORDERS where O_CUSTKEY <= 5;
select * from ORDERS where O_CUSTKEY > 745;
select * from ORDERS where O_CUSTKEY >= 745;
select * from ORDERS where O_CUSTKEY = 567;
select * from ORDERS where O_CUSTKEY = 0;
select * from ORDERS where O_CUSTKEY = 751;

select O_ORDERKEY, avg(O_TOTALPRICE) from ORDERS; -- error, mixed select
select avg(O_TOTALPRICE), min(O_TOTALPRICE), max(O_TOTALPRICE) from ORDERS where O_TOTALPRICE >= 100000;

select * from ORDERS, CUSTOMER, NATION where O_CUSTKEY = C_CUSTKEY and C_NATIONKEY = N_NATIONKEY and N_NAME <> 'INDIA';

create table test (name varchar(10));
insert into test values ('''\n\r\t\');
insert into test values ('\n\n\n\n\n\n'); -- error, too long (\n is interpreted literally)
select * from test where name like '%\';
select * from test where name like '%\\'; -- the same as above
insert into test values ('%%__\\''');
select * from test where name like '\%\%\_\_\\\\''';
insert into test values (null);
select count(name) from test; -- 2
drop table test;

create table t1 (f float, d date, s char(10)); create table t2 (s char(5), f float, d date);
insert into t1 values (1, '2019-01-01', '1'), (3, '2019-01-03', '3'), (5, '2019-01-05', '5'), (7, '2019-01-07', '7');
insert into t2 values ('2', 2, '2019-01-02'), ('4', 4, '2019-01-04'), ('6', 6, '2019-01-06'), ('8', 8, '2019-01-08');
select * from t1, t2 where t1.f < t2.f; select * from t1, t2 where t2.s > t1.s; select * from t1, t2 where t2.d > t1.d;
select * from t2, t1 where t1.f < t2.f; select * from t2, t1 where t2.s > t1.s; select * from t2, t1 where t2.d > t1.d;
select * from t1, t2 where t1.f <> t2.f and t1.s <> t2.s; -- equivalent to no condition
drop table t1; drop table t2;

create table t1 (f float, d date, s varchar(10)); create table t2 (s varchar(5), f float, d date); -- like above, but use varchar, some optimization may fail
insert into t1 values (1, '2019-01-01', '1'), (3, '2019-01-03', '3'), (5, '2019-01-05', '5'), (7, '2019-01-07', '7');
insert into t2 values ('2', 2, '2019-01-02'), ('4', 4, '2019-01-04'), ('6', 6, '2019-01-06'), ('8', 8, '2019-01-08');
select * from t1, t2 where t1.f < t2.f; select * from t1, t2 where t2.s > t1.s; select * from t1, t2 where t2.d > t1.d;
select * from t2, t1 where t1.f < t2.f; select * from t2, t1 where t2.s > t1.s; select * from t2, t1 where t2.d > t1.d;
select * from t1, t2 where t1.f <> t2.f and t1.s <> t2.s; -- equivalent to no condition
drop table t1; drop table t2;

create table test (c char(10), v1 varchar(20), v2 varchar(30));
insert into test values ('hello', 'hello', 'world');
insert into test values ('world', 'hello', 'hello');
select * from test where c = v1 and v1 = c;
select * from test where v1 = v2;
drop table test;