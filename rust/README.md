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
indexes on the same Euclidean workload.

Check the untouched starter without executing TODOs:

```sh
cargo check -p vector-core-starter
cargo check -p vector-datafusion-starter
```

Validate the completed reference:

```sh
cargo test -p vector-core
cargo test -p vector-datafusion
cargo run --release -p vector-core --example recall
```

The workspace uses the stable Rust channel from `rust-toolchain.toml` and pins
course dependencies in `Cargo.lock`.
