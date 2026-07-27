# Use the Index from DataFusion SQL

> **Chapter ID:** `VDB-SQL`
>
> **Prerequisite:** `VDB-EXACT` and one ANN chapter
>
> **Status:** executable reference preview on DataFusion 54.1.0

Before this chapter, DataFusion evaluates a distance for every row and runs a
generic top-k sort. After it, a custom table provider can accept the compatible
sort requirement and execute the selected vector index directly.

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
2. **I2 — Ordered output:** `VectorIndexScanExec` returns its candidate results
   in the requested distance order with deterministic ties.
3. **I3 — Visible plan:** `EXPLAIN` names the index, metric, query dimension,
   and pushed limit.
4. **I4 — Exact fallback:** filters, multiple sort keys, non-literal queries,
   metric mismatches, and wrong directions retain DataFusion's exhaustive
   `SortExec` path.

The ANN index intentionally changes top-k selection from exhaustive to
approximate for a compatible limited query; the plan makes that choice visible.
Filtering after an ANN top-k is not equivalent to filtering before it, so this
preview refuses filtered pushdown rather than claiming unsafe semantics.

## Checkpoints

1. Convert bulk-loaded rows to one Arrow `RecordBatch` and build a core index.
2. Return a full `VectorScanExec` from `TableProvider::scan`.
3. Recognize the physical scalar function in `try_pushdown_sort`.
4. Advertise the accepted ordering and receive `k` through `with_fetch`.
5. Take the selected row offsets from Arrow columns and stream one result batch.
6. Prove each unsafe pattern keeps the exact fallback.

## Verification

Run:

```sh
cargo test -p vector-datafusion
cargo run -p vector-datafusion --example sql
```

The example prints the baseline `SortExec`, the pushed-down
`VectorIndexScanExec`, and results. Stop when I1–I4 hold. Do not add filter
pushdown, SQL DDL, persistence, or an HTTP server.

Explain back why the adapter uses DataFusion's sort-pushdown API instead of a
fork, why the schema order affects limit propagation, and give a two-row
counterexample showing that post-filtering ANN results is unsafe.
