# Rust Course Map

The Rust course is a short, cumulative path whose chapter boundaries follow
concept density rather than calendar days. Its current state is an executable
reference preview; learner checkpoint refs remain a release prerequisite.

| ID | Capability gained | Prerequisite | Implementation evidence |
| --- | --- | --- | --- |
| `VDB-EXACT` | Fixed-dimension metrics and deterministic exact top-k | None | `rust/vector-core/src/{dataset,metric,flat,search}.rs` |
| `VDB-EVAL` | Seeded ground truth, recall, and latency measurement | `VDB-EXACT` | `rust/vector-core/examples/recall.rs` |
| `VDB-SQL` | Match safe top-k SQL and establish the SQLLogicTest ladder | `VDB-EVAL` | `rust/vector-datafusion` |
| `VDB-IVF` | Seeded k-means, inverted lists, and probe-controlled ANN | `VDB-SQL` | `rust/vector-core/src/ivf.rs` and `vector.02-ivfflat.slt` |
| `VDB-NSW` | Incremental single-layer proximity graph search | `VDB-IVF` | `rust/vector-core/src/{graph,nsw}.rs` and `vector.03-nsw.slt` |

Every chapter names its required tests and stop condition. Persistence, online
mutation, filtered ANN, and an HTTP API are follow-up projects, not hidden
requirements.
