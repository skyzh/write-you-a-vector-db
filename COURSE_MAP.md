# Rust Course Map

The Rust course is a cumulative two-day path. Day 1 establishes the table and
optimizer rule before Day 2 implements the first approximate index.

| Day | Capability gained | Prerequisite | Learner implementation |
| --- | --- | --- | --- |
| 1 | Validated in-memory vectors, Arrow table construction, DataFusion scan extension, and safe vector-index matching | None | `rust/vector-starter/core/src/dataset.rs` and `rust/vector-starter/datafusion/src/lib.rs` |
| 2 | Seeded k-means, inverted lists, probe-controlled ANN, and recall measurement | Day 1 | `rust/vector-starter/core/src/{ivf,search}.rs` |

The matching `vector-core` and `vector-datafusion` crates contain the completed
reference. Graph indexes, persistence, online mutation, filtered ANN, and an
HTTP API are follow-up projects rather than hidden requirements.
