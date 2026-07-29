# Rust Course Map

Day 1 establishes the table and optimizer rule before later approximate-index
chapters.

| Day | Capability gained | Prerequisite | Learner implementation |
| --- | --- | --- | --- |
| 1 | Validated in-memory vectors, Arrow table construction, DataFusion scan extension, and safe vector-index matching | None | `rust/vector-core-starter/src/dataset.rs` and `rust/vector-datafusion-starter/src/lib.rs` |

The matching `vector-core` and `vector-datafusion` crates contain the completed
reference. Approximate indexes, persistence, online mutation, filtered ANN,
and an HTTP API are follow-up projects rather than hidden requirements.
