# Build an In-Memory Vector Table and Match Its Index

> **Chapter 1**
>
> Start from the two `*-starter` crates. Finish with an exact SQL top-k query, an Arrow-backed table, and a safe
> DataFusion optimizer rule that can select a vector index.

Your first query asks for the three rows closest to one query vector:

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0])
LIMIT 3;
```

Start from the exact plan. Scan every row, compute its distance to the query, and keep the nearest three with a bounded
top-k sort:

```text
SortExec: TopK(fetch=3), ...
  VectorScanExec: rows=..., fetch=None
```

This plan is correct for every valid query shape because it does not skip any rows. A vector index can avoid scanning the
whole collection, but only when the query and the index describe the same ranking. The matcher must check the distance
function, embedding column, literal query vector, sort direction, dimension, and `LIMIT`; a mismatch keeps the exact
plan.

After your rule recognizes a compatible index, the physical plan becomes:

```text
SortExec: TopK(fetch=3), ...
  VectorIndexScanExec: index=flat, metric=Cosine, query_dim=3, fetch=Some(3), ordered=false
```

With `FlatIndex`, you can confirm both the exact result and the matched physical plan. In Chapter 2, `index=ivf_flat` will
appear behind the same rule.

## Build the Exact Path and Matcher in Rust

You will modify:

```text
rust/vector-starter/core/src/dataset.rs
rust/vector-starter/datafusion/src/lib.rs
```

Metric math, a `FlatIndex` that checks every vector, Arrow result execution, and all tests are ready for you to use. You
will build the storage and extension boundary around them: validated vectors, an Arrow-backed table, a physical scan, and
a rule that recognizes one safe top-k shape. Do not modify public APIs or tests.

## What Must Hold, and What Breaks If It Doesn't

A valid dataset must be non-empty, have a fixed dimension greater than
zero, and contain only finite `f32` vectors. If you accept an empty or
variable-dimension dataset, every downstream similarity check will
silently compare mismatched shapes.

Every row needs a unique external `id`. The same row offset must refer
to the same row in both the in-memory dataset and the Arrow batch. If
an `id` appears twice, the index will return the wrong payload for a
matching row.

The Arrow table schema must be `id: UInt64`, `payload: Utf8`,
`embedding: FixedSizeList<Float32>` with the declared dimension. A
mismatched schema silently breaks every reader that expects those types.

An index scan is selected only for a supported distance expression over
`embedding`, a literal query vector, a compatible metric and direction,
and a valid dimension. If the rule fires on an unsupported expression,
DataFusion will pass the wrong operator to the index and return
incorrect results.

Filters, multiple sort keys, non-literal vectors, wrong metrics, wrong
directions, and invalid query vectors must stay on `VectorScanExec`
with DataFusion's exact sort. If the optimizer tries to use the index
for these, correctness depends on an operation the index does not
perform.

DataFusion owns ordering: the vector scan must return rows in the
requested order, or DataFusion retains its bounded sort after the index
selects candidates. If you return rows out of order, an upstream `LIMIT`
will silently produce the wrong result set.

## Checkpoint 1: Validate the In-Memory Dataset

Implement the three TODOs in `vector-starter/core/src/dataset.rs`.

`Dataset::try_new` reads the first row to establish dimension, rejects an empty dataset or zero-dimensional vector, then
checks every row for equal length and finite components. Store the vectors as `Arc<[Vec<f32>]>`; later exact and
approximate indexes can cheaply share immutable data.

`validate_for_metric` rejects zero-norm stored rows for cosine distance. `validate_query` checks dimension, finiteness,
and the same cosine boundary for a query. Use the existing `VectorError` variants rather than panicking on input.

**Prediction:** For a two-dimensional dataset, should query `[1.0]` reach the DataFusion optimizer? It should be rejected
at the vector boundary; allowing a mismatched literal into an index scan would make the plan claim a contract the index
cannot satisfy.

Use the starter's `FlatIndex` to exercise these checks:

```sh
cd rust
cargo test -p vector-core-starter --test indexes flat_search_is_deterministic_and_validates_queries
cargo test -p vector-core-starter --test indexes cosine_rejects_zero_norm_vectors
```

The exact-search and top-k helpers are already in place, so you can keep this checkpoint focused on the validation
boundary.

## Checkpoint 2: Turn Rows into an Arrow Table

Implement `VectorTable::try_new` in `vector-starter/datafusion/src/lib.rs`.

### Preserve Identity

First reject duplicate external IDs with a `HashSet`. Then build `Dataset` from the embeddings in exactly the same row
order. If Arrow batch row 4 and dataset row 4 refer to different inputs, an index will return the wrong payload even when
its search result is otherwise correct.

Build the selected `IndexConfig` over that dataset. Chapter 1 passes `IndexConfig::Flat`; Chapter 2 will pass
`IndexConfig::IvfFlat` without changing table construction.

### Build This Course's Arrow Table

This course's engine uses a fixed three-column table schema. The index operates
over the single `embedding` column; arbitrary schemas are out of scope.

DataFusion executes over Arrow arrays. Construct this schema:

```text
id         UInt64
payload    Utf8
embedding  FixedSizeList<Float32, dimension>
```

`FixedSizeListArray` stores all embedding components in one flat `Float32Array`; the list width tells Arrow where each
row begins and ends. For two three-dimensional rows, the child values are laid out as:

```text
[x0, y0, z0, x1, y1, z1]
 `---row 0--' `---row 1--'
```

Use `i32::try_from(dataset.dimension())` for Arrow's list width and return a plan error if the dimension cannot fit.
Create `UInt64Array`, `StringArray`, and `FixedSizeListArray`, then assemble one `RecordBatch` with the schema.

**Prediction:** What breaks if you sort the IDs before creating their Arrow array but leave embeddings in insertion
order? Trace the row offset returned by an index to the payload DataFusion would emit.

Run the duplicate-ID boundary test:

```sh
cargo test -p vector-datafusion-starter --test sql table_rejects_duplicate_ids
```

## Checkpoint 3: Expose a TableProvider and Exact Scan

DataFusion asks a `TableProvider` for a physical plan through `scan`. Implement the TODO in that method by creating the
existing `VectorScanExec` in `ScanMode::Full`.

Pass through:

- the Arrow batch and selected core index;
- DataFusion's requested projection and limit;
- the session's `vector_search.ordered` option; and
- no ordering yet, because the initial scan has not accepted a sort.

The executor uses `project_schema` to preserve the requested column order, `take` to build result arrays from
row offsets, and `MemoryStream` to emit one batch. In full mode it returns ordinary table rows. DataFusion evaluates the
distance function and exact `SortExec` above that scan.

At this point, DataFusion can read your table and return exact top-k results. Run the query once and inspect how the scan,
distance expression, and bounded sort fit together.

## Checkpoint 4: Match One Safe Vector Ordering

Implement `match_vector_order` and `try_pushdown_sort`. DataFusion calls the latter while planning the physical query.

The matcher accepts only all of the following:

1. exactly one `PhysicalSortExpr`;
2. a supported scalar function: Euclidean `array_distance`/`list_distance`, `cosine_distance`, or
   `inner_product`/`dot_product`;
3. ascending order for Euclidean/cosine or descending order for dot product;
4. one `Column` and one `Literal` argument, allowing either argument order;
5. the column named `embedding` in the projected schema;
6. a literal vector that `scalar_vector` can convert to finite `f32` values;
7. query dimension equal to the index dataset; and
8. a nonzero cosine query.

`uncast` and `scalar_vector` are already implemented. They remove harmless cast wrappers and decode list literals backed
by integer, `f32`, or `f64` Arrow arrays. Use them to build a conservative matching rule.

When matching fails, return `SortOrderPushdownResult::Unsupported`; DataFusion keeps the exact scan and sort. When it
succeeds, clone the scan into `ScanMode::Vector { query }`, retain the requested ordering, and return
`SortOrderPushdownResult::Exact`.

**Prediction:** A cosine index exists, but the query uses `array_distance`. Both functions accept the same vector shapes.
Should the rule select the index? No—the metric changes ranking, so I4 requires exact fallback.

Run the positive and negative plan tests:

```sh
cargo test -p vector-datafusion-starter --test sql compatible_top_k_uses_vector_index_scan_and_keeps_sort
cargo test -p vector-datafusion-starter --test sql unsafe_sort_shapes_are_not_lowered
cargo test -p vector-datafusion-starter --test sql filter_keeps_datafusion_exact_fallback
cargo test -p vector-datafusion-starter --test sql dot_product_requires_descending_order
```

## Checkpoint 5: Push LIMIT Without Stealing ORDER BY

Implement `ExecutionPlan::with_fetch`. DataFusion calls it after sort pushdown and passes `LIMIT k`.

DataFusion 54.1.0 does not expose supported SQL optimizer hints for choosing this plan. Register
`vector_search.ordered` as a session option instead; DataFusion reads its value whenever it generates a new physical plan.
This keeps the SQL query portable while making the executor's ordering guarantee explicit for the session.

Clone the scan and store the new fetch value. When the session option is `ordered=true`, return the scan directly; this mode
is valid only when the selected index returns rows in the accepted order. If no ordering or no fetch exists, also return
the scan.

For the default `ordered=false` path, clear the scan's claimed ordering property and wrap it in DataFusion's
`SortExec::new(ordering, scan).with_fetch(Some(k))`. The index chooses candidate row offsets; DataFusion still owns
nearest-first SQL output.

This detail prevents a subtle optimizer bug. Claiming exact ordering while an approximate executor returns candidates in
heap or traversal order can produce the right set in the wrong order.

Verify both modes and the end-to-end SQL file:

```sh
cargo test -p vector-datafusion-starter --test sql ordered_session_mode_allows_sort_elision
cargo test -p vector-datafusion-starter --test sqllogictest day1_table_and_optimizer_sql
```

The SQLLogicTest asserts physical operators as well as rows. Its filtered case must remain exact; setting ordered mode may
remove the generic sort, and setting it back to `false` must restore that sort in the next generated plan.

## Review Your Chapter 1 Result

After the two core tests, all `sql.rs` tests, and the Chapter 1 SQLLogicTest pass, choose one query and explain:

- how an input row becomes a dataset offset and three aligned Arrow arrays;
- where DataFusion performs exact distance, top-k, and final ordering;
- which comparison prevents a cosine query from using a Euclidean index;
- why a column-to-column distance expression stays exact; and
- how the same matching rule can later reach an approximate index without weakening exact fallback.

Keep your Chapter 1 changes in the two files named at the start. IVFFlat, filtered pushdown, DDL, and persistence remain
outside this chapter.

{{#include copyright.md}}
