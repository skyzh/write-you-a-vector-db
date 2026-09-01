# Rust Vector Search Course

This Cargo workspace separates starter and reference trees:

```text
vector-starter/
  core/          package: vector-core-starter
  datafusion/    package: vector-datafusion-starter
vector/
  core/          package: vector-core
  datafusion/    package: vector-datafusion
```

Chapter 1 makes vector-index selection observable from SQL. Chapter 2 implements
IVFFlat behind that unchanged optimizer boundary, and Chapter 3 adds NSW graph
search. Chapter 4 adds HNSW hierarchy, Chapter 5 adds residual product
quantization and exact reranking to IVFFlat, and Chapter 6 benchmarks all five
indexes on the external SIFT1M workload with first-neighbor rank recall.

Before implementing Chapter 1, launch the supplied product shell:

```sh
cargo run -p vector-datafusion --example sql
```

The shell starts with an empty session and accepts one SQL statement per line.
Follow the product-tour chapter to create and populate an ordinary in-memory
table, compare a query and `EXPLAIN`, attach an index with names chosen in SQL,
then run the identical query again. You do not need to inspect or modify the
completed reference example.

Check the untouched starter without executing TODOs:

```sh
cargo check -p vector-core-starter
cargo check -p vector-datafusion-starter
```

Validate the completed reference:

```sh
cargo test -p vector-core
cargo test -p vector-datafusion
cargo run --release -p vector-core --example recall -- /absolute/path/to/sift1M
```

The workspace uses the stable Rust channel from `rust-toolchain.toml` and pins
course dependencies in `Cargo.lock`.
