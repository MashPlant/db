use orderDB;

select sum(C_CUSTKEY), sum(C_ACCTBAL) from CUSTOMER;
alter table CUSTOMER drop C_ADDRESS;
alter table CUSTOMER add foo char(10) not null default 'foo';
select sum(C_CUSTKEY), sum(C_ACCTBAL) from CUSTOMER;

create table test (i int, v varchar(10));

alter table test add b bool;

insert into test values (1, 'hello', true);
alter table test add f bool not null; -- error, f will be null
alter table test add f float default 233;
insert into test values (0, 'world', false);
select * from test;

alter table test drop b; alter table test drop v; alter table test drop f;
alter table test drop i; -- error, col num will be 0

drop table test;

create table test1 (a int, b int, primary key(a));
create table test2 (v1 varchar(10), v2 varchar(10), f_a int, f_b int, foreign key(f_a) references test1(a));
alter table test1 add primary key (a, a); -- error, dup col
alter table test1 add primary key (a); -- error, dup constraint
alter table test1 drop a; -- error, there is foreign link to a
alter table test1 add primary key (b); -- error, a will not be unique
alter table test1 drop primary key (a); -- error, a will not be unique
drop table test2;
drop table test1;

create table test (a int, b int);
insert into test values (1, 1), (1, 2);
alter table test add primary key(a, b);
alter table test drop b; -- error, a will be duplicate
alter table test drop primary key (b); -- error, a is duplicate
alter table test drop primary key (a, b);
alter table test add primary key (a); -- error, a is duplicate
alter table test add c int;
alter table test drop primary key(c); -- error, c is not primary
alter table test add primary key(c); -- error, c is null
drop table test;

