# Build an In-Memory Vector Table and Match Its Index

> **Day 1**
>
> Start from the two `*-starter` crates. Finish with an exact SQL top-k query, an Arrow-backed table, and a safe
> DataFusion optimizer rule that can select a vector index before Day 2 implements IVFFlat.

DataFusion already implements vector distance expressions, sorting, and `LIMIT`. You will not reimplement exact
k-nearest-neighbor execution. Instead, this chapter builds the storage and extension boundary that the approximate index
needs: validated vectors, a `TableProvider`, a physical scan, and a rule that recognizes one safe top-k shape.

You will modify:

```text
rust/vector-starter/core/src/dataset.rs
rust/vector-starter/datafusion/src/lib.rs
```

The starter supplies metric math, a small `FlatIndex` used as an oracle and index-selection test double, Arrow result
execution, and all tests. Do not modify public APIs or tests.

## The Query

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0])
LIMIT 3;
```

Without index matching, DataFusion evaluates `cosine_distance` for every scanned row and applies its own bounded
`SortExec: TopK`. That is the correct exact plan and remains the fallback throughout the course.

After your rule recognizes a compatible index, the physical plan becomes:

```text
SortExec: TopK(fetch=3), ...
  VectorIndexScanExec: index=flat, metric=Cosine, query_dim=3, fetch=Some(3), ordered=false
```

Day 1 uses the supplied flat implementation so the matching rule can be completed and tested before any ANN algorithm
exists. On Day 2, `index=ivf_flat` will appear behind the same rule.

## Invariants

1. **I1 — Valid vectors:** a dataset is non-empty, has nonzero fixed dimension, and contains only finite `f32` values.
2. **I2 — Stable identity:** each external `id` is unique, while a core row offset identifies the same row in the
   dataset and Arrow batch.
3. **I3 — Faithful Arrow shape:** the table schema is `id: UInt64`, `payload: Utf8`, and
   `embedding: FixedSizeList<Float32>` with the dataset dimension.
4. **I4 — Safe match:** an index scan is selected only for one supported distance expression over `embedding`, a literal
   query vector, a compatible metric and direction, and a valid dimension.
5. **I5 — Exact fallback:** filters, multiple sort keys, non-literal vectors, wrong metrics, wrong directions, and invalid
   query vectors remain on `VectorScanExec` plus DataFusion's exact sort.
6. **I6 — SQL owns ordering:** unless the executor explicitly promises ordered output, DataFusion retains its bounded
   sort after the index selects candidates.

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

The starter's supplied `FlatIndex` exercises these checks:

```sh
cd rust
cargo test -p vector-core-starter --test indexes flat_search_is_deterministic_and_validates_queries
cargo test -p vector-core-starter --test indexes cosine_rejects_zero_norm_vectors
```

The tests also cover the supplied exact oracle. Your responsibility in this checkpoint is the validation boundary, not
its top-k heap.

## Checkpoint 2: Turn Rows into an Arrow Table

Implement `VectorTable::try_new` in `vector-starter/datafusion/src/lib.rs`.

### Preserve Identity

First reject duplicate external IDs with a `HashSet`. Then build `Dataset` from the embeddings in exactly the same row
order. If Arrow batch row 4 and dataset row 4 refer to different inputs, an index will return the wrong payload even when
its search result is otherwise correct.

Build the selected `IndexConfig` over that dataset. Day 1 passes `IndexConfig::Flat`; Day 2 will pass
`IndexConfig::IvfFlat` without changing table construction.

### Define the Schema

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
supplied `VectorScanExec` in `ScanMode::Full`.

Pass through:

- the Arrow batch and selected core index;
- DataFusion's requested projection and limit;
- the session's `vector_search.ordered` option; and
- no ordering yet, because the initial scan has not accepted a sort.

The supplied executor uses `project_schema` to preserve the requested column order, `take` to build result arrays from
row offsets, and `MemoryStream` to emit one batch. In full mode it returns ordinary table rows. DataFusion evaluates the
distance function and exact `SortExec` above that scan.

This is the Rust equivalent of the C++ course's insert/scan checkpoint, with Arrow arrays replacing serialized BusTub
tuples and `TableProvider::scan` replacing `SeqScanExecutor`.

## Checkpoint 4: Match One Safe Vector Ordering

Implement `match_vector_order` and `try_pushdown_sort`. DataFusion calls the latter during physical sort pushdown, before
Day 2's algorithm runs.

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

`uncast` and `scalar_vector` are supplied. They remove harmless cast wrappers and decode list literals backed by integer,
`f32`, or `f64` Arrow arrays. The learning task is to compose those helpers into a conservative rule.

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

Clone the scan and store the new fetch value. If the session option says `ordered=true`, the executor promises that its
output already satisfies the accepted ordering, so return the scan directly. If no ordering or no fetch exists, also
return the scan.

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

The SQLLogicTest asserts physical operators as well as rows. Its filtered case must remain exact; its explicit ordered
session may remove the generic sort.

## Done When

Day 1 is complete when the two core tests, all `sql.rs` tests, and the Day 1 SQLLogicTest pass. Explain back, using one
query:

- how an input row becomes a dataset offset and three aligned Arrow arrays;
- where DataFusion performs exact distance, top-k, and final ordering;
- which comparison prevents a cosine query from using a Euclidean index;
- why a column-to-column distance expression stays exact; and
- why sort pushdown must be working before an ANN implementation can be tested from SQL.

Do not implement a custom exact executor, ANN algorithm, filtered pushdown, or DDL in this chapter. DataFusion already
provides exact expression and top-k execution; Day 2 will supply the first approximate candidate selector.

{{#include copyright.md}}
