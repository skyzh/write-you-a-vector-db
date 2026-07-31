# Rust Course Map

The Rust course is cumulative. Its five chapters establish the table and optimizer
rule, add partition-based and graph-based approximate search, and compare every
index on one benchmark workload.

| Chapter | Capability gained | Prerequisite | Files you will change |
| --- | --- | --- | --- |
| 1 | Validated in-memory vectors, Arrow table construction, DataFusion scan extension, and safe vector-index matching | None | `rust/vector-starter/core/src/dataset.rs` and `rust/vector-starter/datafusion/src/lib.rs` |
| 2 | Seeded k-means, inverted lists, probe-controlled ANN, and recall measurement | Chapter 1 | `rust/vector-starter/core/src/{ivf,search}.rs` |
| 3 | Best-first graph traversal, reciprocal bounded-degree insertion, and search-width control | Chapter 2 | `rust/vector-starter/core/src/{graph,nsw}.rs` |
| 4 | Seeded hierarchical layers, greedy upper-layer routing, and layer-zero beam search | Chapter 3 | `rust/vector-starter/core/src/{graph,hnsw}.rs` |
| 5 | Fair recall and latency measurement across exact, IVFFlat, NSW, and HNSW search | Chapter 4 | `rust/vector-starter/core/examples/recall.rs` |

Optional follow-up:

| Chapter | Capability gained | Prerequisite | Files you will change |
| --- | --- | --- | --- |
| 6 | Residual product quantization, asymmetric lookup-table scoring, exact reranking, and compressed-representation accounting | Chapter 5 | `rust/vector-starter/core/src/pq.rs` |

The matching `vector-core` and `vector-datafusion` crates contain the completed
reference. Persistence, online mutation, filtered ANN, and an HTTP API remain
outside these chapters.
