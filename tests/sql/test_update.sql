use orderDB;

update LINEITEM set L_LINENUMBER = -L_LINENUMBER where L_LINENUMBER > 4;
update LINEITEM set L_LINENUMBER = -L_LINENUMBER where L_LINENUMBER < -4;

update CUSTOMER set C_CUSTKEY = -C_CUSTKEY; -- error, there are foreign link to customer
update LINEITEM set L_LINENUMBER = 0; -- error, dup primary key (one update will success)

update LINEITEM set L_LINENUMBER = L_LINENUMBER + 1 - 2 * 3 / 4 % 5 - 1000000; -- note that / is fdiv, % is fmod

create table test(i int, v char(10), b bool, primary key (v, b), unique(i));
insert into test values (1, 'hello', true);
update test set b = i < 0 and v like 'he_lo';
update test set b = i < 0 or v like 'he_lo';
update test set b = i is not null and v is not null; -- now the only key in test is (1, 'hello', true)
insert into test values (2, 'hello', false);
update test set i = 1 where i = 2; -- error, dup i
update test set b = true where i = 2; -- error, dup composite primary key
drop table test;

create table test (v1 varchar(2) not null, v2 varchar(2));
insert into test values ('v1', 'v2');

update test set v1 = 'long', v2 = 'v2'; -- error, and `lit2varchar` should never be called, belows are the same
update test set v1 = 'v1', v2 = 'long'; -- error
update test set v1 = 'v1', v2 = 233; -- error
update test set v1 = 233, v2 = 'v2'; -- error
update test set v1 = null, v2 = 'long'; -- error, and `free_varchar` should never be called
update test set v1 = 'v2', v2 = null;
select * from test;
drop table test;