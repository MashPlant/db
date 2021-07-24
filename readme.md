A database management system implemented in Rust from scratch. [Chinese version readme](readme-cn.md).

# Features

- Query optimization based on B+ tree
- Multi-table join optimization based on sorting and binary search
- Aggregate query: supports `avg`, `sum`, `max`, `min`, `count` keywords to aggregate the results of `select`
- Fuzzy matching: supports `like` keywords, wildcard characters `%` and `_`, and escape characters
- `DATE` data type
- Supports `unique` and `check` constraint
- `insert` supports specifying the column to be inserted, and the `set` clause of `update` supports complex expressions
- Colored REPL

<img src="report/repl.png" width="400"/>

# Building and Testing

Requires nightly Rust compiler. Tested on `rustc 1.54.0-nightly`, and a newer version is also welcomed.

Execute `cargo run --bin db --release` to run the database REPL.

[tests](tests) crate contains integration tests. Database creation takes a relatively long time and other tests depend on it, so it needs to be executed separately in advance: `cargo test -p tests create --release - --ignored`. Then execute `cargo test -p tests --release`.

[makefile](makefile) is used for code coverage test. Installation prerequisites:

- `cargo-tarpaulin` (`cargo install cargo-tarpaulin`)
- `pycobertura` (`pip install pycobertura`)
- The browser excutable specified in the `BROWSER` variable

Execute `make` to perform code coverage test. An example looks like:

<img src="report/coverage.png" width="400"/>

# Project Structure

The project divides logical components with Rust's crates as the boundary. The dependencies between crates are like:

<img src="report/arch.png" width="250"/>

- [common](common): Provide common functions and data structures, such as error handling mechanisms
- [physics](physics): Define the data structures of different physics pages of the database
- [syntax](syntax): Parse SQL statements into AST based on the parser generator [lalr1](https://github.com/MashPlant/lalr1) written by myself
- [db](db): Define the core interfaces of the database, and implement some modification operations not requiring the index
- [index](index): Implement index based on B+ tree, and implement some modification operations requiring the index
- [query](query): Implement the four kinds of queries, QRUD
- [driver](driver): Top-level interfaces and executables

See [report.pdf](report/report.pdf) for a (Chinese) project report。