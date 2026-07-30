# Rust Course Map

The Rust course is cumulative. These first two chapters establish the table and
optimizer rule, then add the first approximate index.

| Chapter | Capability gained | Prerequisite | Files you will change |
| --- | --- | --- | --- |
| 1 | Validated in-memory vectors, Arrow table construction, DataFusion scan extension, and safe vector-index matching | None | `rust/vector-starter/core/src/dataset.rs` and `rust/vector-starter/datafusion/src/lib.rs` |
| 2 | Seeded k-means, inverted lists, probe-controlled ANN, and recall measurement | Chapter 1 | `rust/vector-starter/core/src/{ivf,search}.rs` |

The matching `vector-core` and `vector-datafusion` crates contain the completed
reference. Later chapters continue with graph indexes. Persistence, online
mutation, filtered ANN, and an HTTP API remain outside these two chapters.
