# Rust Course Map

The Rust course begins with a supplied product tour, then its six required
chapters establish the table and optimizer rule, add partition-based and
graph-based approximate search, compress candidate scoring with IVF-PQ, and
compare all five indexes on one workload.

| Chapter | Capability gained | Prerequisite | Files you will change |
| --- | --- | --- | --- |
| Tour | Create and populate an in-memory table, run nearest-neighbor SQL, attach an IVFFlat index to a selected vector column, and observe the plan change | None | None; use the supplied `vector-datafusion` shell |
| 1 | Validated in-memory vectors, Arrow table construction, DataFusion scan extension, and safe vector-index matching | None | `rust/vector-starter/core/src/dataset.rs` and `rust/vector-starter/datafusion/src/lib.rs` |
| 2 | Seeded k-means, inverted lists, probe-controlled ANN, and recall measurement | Chapter 1 | `rust/vector-starter/core/src/{ivf,search}.rs` |
| 3 | Best-first graph traversal, reciprocal bounded-degree insertion, and search-width control | Chapter 2 | `rust/vector-starter/core/src/{graph,nsw}.rs` |
| 4 | Seeded hierarchical layers, greedy upper-layer routing, and layer-zero beam search | Chapter 3 | `rust/vector-starter/core/src/{graph,hnsw}.rs` |
| 5 | Residual product quantization, asymmetric lookup-table scoring, exact reranking, and compressed-representation accounting | Chapter 4 | `rust/vector-starter/core/src/pq.rs` |
| 6 | Fair recall and latency measurement across Flat, IVFFlat, NSW, HNSW, and IVF-PQ on one Euclidean workload | Chapter 5 | `rust/vector-starter/core/examples/recall.rs` |

The matching `vector-core` and `vector-datafusion` crates contain the completed
reference. The tour runs one supplied reference example without asking you to
inspect it. Its bounded SQL bridge accepts named or qualified in-memory tables
and multiple distinct index attachments, but persistence, online index
maintenance, filtered ANN, general catalog semantics, and an HTTP API remain
outside these chapters.
