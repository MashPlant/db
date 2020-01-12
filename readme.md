以下摘自报告：

本项目使用rust实现。执行`cargo run --bin db --release`运行数据库repl，执行`cargo test -p tests --release`进行测试，执行`make`进行代码覆盖率测试。要求nightly版本的rust编译器，版本越新越好；为了执行代码覆盖率测试，需先安装`cargo-tarpaulin`(安装方法为`cargo install cargo-tarpaulin`)和`pycobertura`(安装方法为`pip install pycobertura`)，并且装有makefile中指定的浏览器。

截至目前为止，rust代码总行数为3580。

实现了以下的额外功能：

- 简单的查询优化
- 多表连接：理论上支持任意多表的连接(只要性能和空间允许)
- 聚集查询：支持`avg`，`sum`，`max`，`min`，`count`关键字，对`select`的结果进行聚集
- 模糊查询：支持`like`关键字，包括通配符`%`和`_`，支持转义字符
- 日期数据类型
- `unique`约束，`check`约束
- `insert`支持指定要插入数据的列，`update`的`set`子句支持复杂的表达式
- 简单的命令行着色

完整报告见[report.pdf](report/report.pdf)。