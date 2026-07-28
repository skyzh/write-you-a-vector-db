# Rust Vector Search Extension

This workspace is the executable reference preview for the Rust course.

The first checkpoint implements validated datasets, Euclidean/cosine/dot
metrics, and deterministic exact top-k search in `vector-core`.

The second checkpoint adds recall measurement and a deterministic in-process
workload. At this point the harness compares exact search with itself; later
chapters replace the candidate index without changing the measurement loop.

Run it with:

```sh
cargo test --workspace
cargo run --release -p vector-core --example recall
```
