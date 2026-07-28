# Rust Vector Search Extension

This workspace is the executable reference preview for the Rust course.

The first checkpoint implements validated datasets, Euclidean/cosine/dot
metrics, and deterministic exact top-k search in `vector-core`.

The second checkpoint adds recall measurement and a deterministic in-process
workload. At this point the harness compares exact search with itself; later
chapters replace the candidate index without changing the measurement loop.

The third checkpoint adds `vector-datafusion`. It matches a compatible SQL
top-k sort to `FlatIndex`, keeps unsupported shapes on DataFusion's exhaustive
path, and establishes the SQLLogicTest runner used by every ANN chapter.

IVFFlat is the first ANN checkpoint. Seeded k-means builds the inverted lists,
`probes` controls the candidate budget, and `vector.02-ivfflat.slt` exercises
the index through the SQL matcher.

Run it with:

```sh
cargo test --workspace
cargo test -p vector-datafusion --test sqllogictest
cargo run --release -p vector-core --example recall
cargo run -p vector-datafusion --example sql
```
