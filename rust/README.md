# Rust Vector Search Extension

This workspace is the executable reference preview for the Rust course.

```text
vector-datafusion  ->  vector-core  ->  Dataset
  Arrow + SQL          exact / IVF      Vec<f32>
  plan pushdown        NSW / HNSW
```

`vector-core` has no Arrow or SQL dependency. `vector-datafusion` exposes a
fixed schema—`id`, `payload`, `embedding`—and recognizes one-column top-k sorts
whose distance function, direction, literal query vector, and table metric all
agree. DataFusion 54.1.0's built-in sort-pushdown rule then replaces its generic
top-k sort with `VectorIndexScanExec` and passes the literal `LIMIT` through
`ExecutionPlan::with_fetch`.

The course establishes this SQL index-matching boundary with exact search
before implementing ANN. Each IVFFlat, NSW, and HNSW chapter then adds a
SQLLogicTest case under `vector-datafusion/tests/slt`, using the same query and
plan contract as the first exact checkpoint.

Run the tests and the SQL plan demonstration:

```sh
cargo test --workspace
cargo test -p vector-datafusion --test sqllogictest
cargo run -p vector-datafusion --example sql
```

Measure recall and latency on a deterministic in-process fixture:

```sh
cargo run --release -p vector-core --example recall
```

The SQL adapter deliberately refuses filters, non-literal query vectors,
multiple sort keys, metric mismatches, and the wrong sort direction. Those
queries keep DataFusion's exhaustive `SortExec` plan. Filtered ANN, persistence,
and online mutation are outside the preview's contract.
