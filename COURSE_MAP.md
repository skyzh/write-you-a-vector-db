# Rust Course Map

The Rust course is a short, cumulative path whose chapter boundaries follow
concept density rather than calendar days. Its current state is an executable
reference preview; learner checkpoint refs remain a release prerequisite.

| ID | Capability gained | Prerequisite | Implementation evidence |
| --- | --- | --- | --- |
| `VDB-EXACT` | Fixed-dimension metrics and deterministic exact top-k | None | `rust/vector-core/src/{dataset,metric,flat,search}.rs` |

Every chapter names its required tests and stop condition. Persistence, online
mutation, filtered ANN, and an HTTP API are follow-up projects, not hidden
requirements.
