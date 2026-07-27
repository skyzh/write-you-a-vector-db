# Where to Go Next

A one-week course can make the central tradeoffs visible, but it cannot turn a teaching system into a production vector
database. The proposed Rust course deliberately stops after an immutable in-memory collection with exact, IVFFlat, HNSW,
and SQL query support.

Once that system works, several extensions make good independent projects.

## Storage and Index Layout

- Map immutable vector and index files directly instead of decoding them into many heap allocations.
- Compare array-of-structures and structure-of-arrays layouts for distance evaluation and graph traversal.
- Add scalar or product quantization, then measure memory, latency, and recall together.
- Rebuild large indexes with bounded memory and resumable checkpoints.

## Query Processing

- Extend the DataFusion adapter with safe filtered top-k pushdown, DDL, and index selection.
- Add hybrid lexical and vector retrieval with an explicit score-combination contract.
- Explore pre-filtering, in-traversal filtering, post-filtering, and adaptive oversampling for selective predicates.
- Add a reranking stage that fetches full-precision vectors only for the final candidates.

## Serving

- Add a thin HTTP or gRPC adapter over the same collection API.
- Define request validation, cancellation, admission control, and graceful shutdown.
- Compare library, SQL, and network measurements without hiding serialization or queueing cost.

## Transactions and Distribution

- Define snapshot semantics across base segments, mutation logs, and index generations.
- Replicate the mutation log and decide when an acknowledged write becomes searchable.
- Shard collections, merge per-shard top-k results, and measure how routing affects recall.
- Move immutable generations to object storage and separate compute from storage.

## Hardware-Aware Search

- Vectorize exact distance calculations with portable SIMD.
- Batch queries to improve cache reuse and throughput without hiding tail latency.
- Compare CPU and GPU search only after including transfer, queueing, and batching costs.
- Profile real embedding dimensions and datasets instead of relying on tiny synthetic vectors.

For any extension, keep the exact implementation as an oracle, state the workload, and report correctness together with
performance. A vector index is useful only when its speedup is attached to a result-quality and lifecycle contract.

## Feedback

The Rust course is available as an executable preview. Feedback about the scope, ordering, datasets, and learner
checkpoints is welcome before the starter/completed refs are published.

[![Join skyzh's Discord Server](discord-badge.svg)](https://skyzh.dev/join/discord)

{{#include copyright.md}}
