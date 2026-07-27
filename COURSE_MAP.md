# Rust Course Map

The Rust course is a short, cumulative path whose chapter boundaries follow
concept density rather than calendar days. Its current state is an executable
reference preview; learner checkpoint refs remain a release prerequisite.

| ID | Capability gained | Prerequisite | Implementation evidence |
| --- | --- | --- | --- |
| `VDB-EXACT` | Fixed-dimension metrics and deterministic exact top-k | None | `rust/vector-core/src/{dataset,metric,flat,search}.rs` |
| `VDB-EVAL` | Seeded ground truth, recall, and latency measurement | `VDB-EXACT` | `rust/vector-core/examples/recall.rs` |
| `VDB-IVF` | Seeded k-means, inverted lists, and probe-controlled ANN | `VDB-EVAL` | `rust/vector-core/src/ivf.rs` |
| `VDB-NSW` | Incremental single-layer proximity graph search | `VDB-EVAL` | `rust/vector-core/src/{graph,nsw}.rs` |
| `VDB-HNSW` | Seeded levels and hierarchical graph traversal | `VDB-NSW` | `rust/vector-core/src/hnsw.rs` |
| `VDB-SQL` | Safe top-k sort pushdown into a vector index | `VDB-EXACT` plus one ANN index | `rust/vector-datafusion` |

Every chapter names its required tests and stop condition. Persistence, online
mutation, filtered ANN, and an HTTP API are follow-up projects, not hidden
requirements.
