^ -- error
; -- error
show database OrderDB; -- error
use OrderDB; -- error
use orderDB;
create table CUSTOMER(id int(10) not null); -- error, duplicate
create table t (id int, id int); -- error, duplicate
create table t (id int(256) not null); -- error, u8 overflow
create table t (id int(255) not null);
insert into t value (2147483648); -- error, i32 overflow
insert into t values (null); -- error
create table t1 (id int(255), CHECK (id IN ('F', 'M'))); -- error, check ty mismatch
create table t1 (id DATE, CHECK (id IN ('2019-01-01')));
select id from t, t1; -- error, ambiguous col
drop table t2; -- error, no such table
drop table t;
drop table t1;

create table t (v varchar(10), unique(v)); -- error, unsupported varchar op
create table t (v varchar(10), primary key (v)); -- error
create table t (v varchar(10) default ''); -- error
create table t (v varchar(10), check (v in (''))); -- error
create table t (v varchar(10));
alter table t add index test_v_idx on(v); -- error
alter table t add primary key (v); -- error
create table t1 (v varchar(10), foreign key (v) references t(v)); -- error
create table t1 (v varchar(10));
alter table t1 add foreign key (v) references t(v); -- error
drop table t;
drop table t1;