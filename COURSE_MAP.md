# Rust Course Map

The Rust course begins with a supplied product tour, then its six required
chapters establish the table and optimizer rule, add partition-based and
graph-based approximate search, compress candidate scoring with IVF-PQ, and
compare all five indexes on the external SIFT1M workload.

| Chapter | Capability gained | Prerequisite | Files you will change |
| --- | --- | --- | --- |
| Tour | Create and populate an in-memory table, run nearest-neighbor SQL, attach an IVFFlat index to a selected vector column, and observe the plan change | None | None; use the supplied `vector-datafusion` shell |
| 1 | Validated in-memory vectors, Arrow table construction, DataFusion scan extension, and safe vector-index matching | None | `vector-db-starter/core/src/dataset.rs` and `vector-db-starter/datafusion/src/lib.rs` |
| 2 | Seeded k-means, inverted lists, probe-controlled ANN, and recall measurement | Chapter 1 | `vector-db-starter/core/src/{ivf,search}.rs` |
| 3 | Best-first graph traversal, reciprocal bounded-degree insertion, and search-width control | Chapter 2 | `vector-db-starter/core/src/{graph,nsw}.rs` |
| 4 | Seeded hierarchical layers, greedy upper-layer routing, and layer-zero beam search | Chapter 3 | `vector-db-starter/core/src/{graph,hnsw}.rs` |
| 5 | Residual product quantization, asymmetric lookup-table scoring, exact reranking, and compressed-representation accounting | Chapter 4 | `vector-db-starter/core/src/pq.rs` |
| 6 | SIFT1M rank-recall and latency measurement across Flat, IVFFlat, NSW, HNSW, and IVF-PQ under one fixed Euclidean contract | Chapter 5 and a local SIFT1M copy | `vector-db-starter/core/examples/recall.rs` |

The matching `vector-core` and `vector-datafusion` crates contain the completed
reference. The tour runs one supplied reference example without asking you to
inspect it. Its bounded SQL bridge accepts named or qualified in-memory tables
and multiple distinct index attachments, but persistence, online index
maintenance, filtered ANN, general catalog semantics, and an HTTP API remain
outside these chapters. The course does not redistribute or download SIFT1M;
Chapter 6 validates files that you acquire from the upstream corpus page.
