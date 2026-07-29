# Rust Vector Search Course

This Cargo workspace contains learner and reference code for Day 1:

| Crate | Role |
| --- | --- |
| `vector-core-starter` | Day 1 dataset-validation TODOs; exact helpers are supplied |
| `vector-datafusion-starter` | Day 1 Arrow table, scan extension, and optimizer-rule TODOs |
| `vector-core` | Completed core reference |
| `vector-datafusion` | Completed DataFusion reference |

Day 1 makes vector-index selection observable from SQL before later
approximate indexes are implemented.

Check the untouched starter without executing TODOs:

```sh
cargo check -p vector-core-starter
cargo check -p vector-datafusion-starter
```

Validate the completed reference:

```sh
cargo test -p vector-core
cargo test -p vector-datafusion
cargo run -p vector-datafusion --example sql
```

The workspace uses the stable Rust channel from `rust-toolchain.toml` and pins
course dependencies in `Cargo.lock`.
