# Rust Vector Search Course

This Cargo workspace separates learner code from completed references:

```text
vector-starter/
  core/          package: vector-core-starter
  datafusion/    package: vector-datafusion-starter
vector/
  core/          package: vector-core
  datafusion/    package: vector-datafusion
```

Day 1 makes vector-index selection observable from SQL. Day 2 implements
IVFFlat behind that unchanged optimizer boundary.

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
