use orderDB;

delete from ORDERS where O_ORDERKEY1 > 0; -- error
delete from ORDERS where order.O_ORDERKEY > 0; -- error

select count(*) from LINEITEM;
delete from LINEITEM where L_ORDERKEY > 15000;
select count(*) from LINEITEM;

delete from CUSTOMER; -- error, there are foreign link to customer