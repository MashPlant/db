use orderDB;

create table test (i int, b bool default true, f float default 233, v char(10) default 'world', d date, check (v in ('hello', 'world')));
desc test;

insert into test (v) values ('foo'); -- error, not in check
insert into test values (19260817, false, 19260817.0, 'hello', '2019-10-01');
select * from test where i = 19260817 and b = false and f = 19260817.0 and v = 'hello' and d = '2019-10-01';
select * from test where i = f and b = b and f = i and v = v and d = d;

create table test1 (i int, b bool, f float, v varchar(10), d date);
insert into test1 values (19260817, false, 19260817.0, 'hello', '2019-10-01');
select * from test, test1 where test.i = test1.f and test.b = test1.b and test.f = test1.i and test.v = test1.v and test.d = test1.d;

insert into test (d, i) values ('2019-10-01', -233);
insert into test values (666);
select * from test;
insert into test (i, b, f, v) values (1, true, 1, '1', '2019-10-01'); -- error, too long
insert into test values (1, true, 1, '1', '2019-10-01', 1); -- error, too long

drop table test;
drop table test1;