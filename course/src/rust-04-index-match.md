# Match a Vector Index from SQL

> **Chapter ID:** `VDB-SQL`
>
> **Prerequisite:** `VDB-EVAL`
>
> **Status:** executable reference preview on DataFusion 54.1.0

Before this chapter, exact search and recall measurement are ordinary Rust
library operations. After it, a custom table provider recognizes a compatible
SQL top-k query and executes the selected vector index. The first checkpoint
uses `FlatIndex`; every later ANN chapter reuses the same SQLLogicTest boundary.

## Target Query

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0])
LIMIT 10;
```

The adapter in `rust/vector-datafusion/src/lib.rs` exposes schema `id, payload,
embedding`. That order is deliberate: DataFusion can append the hidden sort
column without inserting a reordering projection between `SortExec` and the
scan, so the literal limit reaches `ExecutionPlan::with_fetch`.

## Contract

1. **I1 — Compatible expression:** lower only one supported distance function
   with one embedding column, one literal query vector, matching dimension and
   metric, and the correct direction (ascending for Euclidean/cosine,
   descending for inner product).
2. **I2 — Ordered output:** `VectorIndexScanExec` returns results in the
   requested distance order with deterministic ties.
3. **I3 — Visible plan:** `EXPLAIN` names the index, metric, query dimension,
   and pushed limit.
4. **I4 — Exact fallback:** filters, multiple sort keys, non-literal queries,
   metric mismatches, and wrong directions retain DataFusion's exhaustive
   `SortExec` path.

The first matched plan still uses exact `FlatIndex`, separating planner
correctness from approximation. IVFFlat, NSW, and HNSW later replace only the
selected index and add their own `.slt` cases. Filtering after an ANN top-k is
not equivalent to filtering before it, so the adapter refuses filtered
pushdown rather than claiming unsafe semantics.

## SQLLogicTest Ladder

The Rust runner follows the legacy C++ course's executable SQL style while
using DataFusion directly. Files under
`rust/vector-datafusion/tests/slt/` contain `query` records for normalized
physical-plan lines and result rows. `tests/sqllogictest.rs` registers the
chapter's fixed fixture, executes each file, and normalizes Arrow results for
the `sqllogictest` runner.

The `EXPLAIN` records assert `VectorIndexScanExec` (or the intended
`VectorScanExec` fallback) from DataFusion's actual physical plan. A result-only
test would not prove that the matcher selected the index.

`vector.01-index-match.slt` proves the exact index path and filtered fallback.
The next three files are added with IVFFlat, NSW, and HNSW, so those algorithms
are exercised through the public SQL boundary instead of only through Rust
unit tests.

## Checkpoints

1. Convert bulk-loaded rows to one Arrow `RecordBatch` and build `FlatIndex`.
2. Return a full `VectorScanExec` from `TableProvider::scan`.
3. Recognize the physical scalar function in `try_pushdown_sort`.
4. Advertise the accepted ordering and receive `k` through `with_fetch`.
5. Take the selected row offsets from Arrow columns and stream one result batch.
6. Add the SQLLogicTest runner and prove each unsafe pattern keeps the fallback.

## Verification

Run:

```sh
cargo test -p vector-datafusion --test sqllogictest
cargo test -p vector-datafusion
cargo run -p vector-datafusion --example sql
```

The example prints the exhaustive `SortExec`, the matched
`VectorIndexScanExec`, and results. Stop when I1–I4 hold with `FlatIndex`. Do not
implement an ANN algorithm, filter pushdown, SQL DDL, persistence, or an HTTP
server in this chapter.

Explain back why matching exact search before ANN makes the later SQLLogicTests
useful, why schema order affects limit propagation, and give a two-row
counterexample showing that post-filtering ANN results is unsafe.
