# Where to Go Next

This short course can make the central tradeoffs visible, but it cannot turn a teaching system into a production vector
database. The course stops after an immutable in-memory collection with exact fallback, IVFFlat, and SQL query support.

## One SQL Query, Many Layers

Vector search does not have to live behind a separate service. A SQL engine can support it through a small set of
extension points: vector values and distance expressions at the interface, exact or approximate indexes underneath, and
a planner rule that connects an `ORDER BY` distance query with `LIMIT k` to the right execution plan. The code at each
boundary can be small; making the boundaries agree is the database work.

Andy Pavlo made a similar observation in
[Databases in 2023: A Year in Review](https://ottertune.com/blog/2023-databases-retrospective): vector search spread quickly
because it can often be added as a new access method and index rather than a new database architecture. This course lets
you see that integration layer by layer, behind one SQL top-k query.

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

## Why This Course Exists

My first close look at vector databases came during my 2023 internship at Neon. Nikita added me to a Slack channel called
`#vector`, where Konstantin was building [pg_embedding](https://github.com/neondatabase/pg_embedding), a PostgreSQL
extension with HNSW support. The project was later discontinued after pgvector added HNSW, but it left me with the question
that became this course: what actually has to change inside a database to make one SQL vector query work?

That question led me to build the original version of this course on BusTub. Thanks to Yuchen, Avery, Ruijie, and the
15-445 course staff for reviewing and merging the
[upstream vector-type change](https://github.com/cmu-db/bustub/pull/682) that made it possible. The Rust and DataFusion
course continues the same investigation with a small in-memory system whose layers can be read end to end.

## Feedback

The first two Rust days include learner starter code, executable references, focused tests, and SQL plan checks.
Feedback about the scope, ordering, datasets, or architecture is welcome.

[![Join skyzh's Discord Server](discord-badge.svg)](https://skyzh.dev/join/discord)

{{#include copyright.md}}
