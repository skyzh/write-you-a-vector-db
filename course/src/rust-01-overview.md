# Build Vector Search in Rust

<div class="warning">

**Course status:** Day 1 is ready to learn from and implement. The repository includes learner starter code, focused
tests, and a separate reference solution.

</div>

In Day 1, you will connect an in-memory vector table to DataFusion and implement the optimizer rule that selects a safe
vector-index scan. This integration comes before ANN algorithms so later index implementations can be tested through SQL
as soon as they work.

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [0.1, 0.2, 0.3])
LIMIT 10;
```

DataFusion already implements vector distance expressions, exact sorting, and `LIMIT`. The course does not ask you to
rebuild exact k-nearest-neighbor execution. The starter supplies a small flat oracle for algorithm tests and optimizer
bring-up; learner work begins at the table and extension boundary.

## Choose the Learner Workspace

The Cargo workspace under `rust/` has paired crates:

```text
vector-core-starter          validated dataset TODOs and supplied exact helpers
vector-datafusion-starter    Day 1 Arrow table and optimizer-rule TODOs

vector-core                  completed core reference
vector-datafusion            completed DataFusion reference
```

The starter keeps the same public APIs, tests, examples, and file layout as the reference. Implement the TODOs in chapter
order. The reference crates are an answer key, not a prerequisite.

From the repository root, check that the untouched starter compiles:

```sh
cd rust
cargo check -p vector-core-starter
cargo check -p vector-datafusion-starter
```

The focused tests initially stop at `todo!` calls. Each chapter names the exact tests that should pass before you move
on.

## One Query, Two Plans

Before index matching, the query is exact:

```text
SortExec: TopK(fetch=10), ...
  VectorScanExec: rows=..., fetch=None
```

`VectorScanExec` emits Arrow rows. DataFusion evaluates the distance function for every row and uses its own bounded sort
to produce the nearest ten.

On Day 1, you implement `ExecutionPlan::try_pushdown_sort`. It accepts only one compatible distance ordering over the
`embedding` column with a literal query vector. `with_fetch` receives `LIMIT k`, and the matched scan asks the selected
index for `k` candidate row offsets:

```text
SortExec: TopK(fetch=10), ...
  VectorIndexScanExec: index=flat, metric=Cosine, query_dim=3, fetch=Some(10), ordered=false
```

The supplied flat index lets you verify this rule before an approximate index exists.

The default plan retains DataFusion's bounded sort. The index selects candidates; `SortExec` owns SQL ordering. The
optional `SET vector_search.ordered = true` promise allows sort elision when the executor guarantees ordered output.

Filters, multiple sort keys, a non-literal query vector, the wrong distance function, the wrong direction, or a dimension
mismatch keep the exact plan. In particular, taking ANN top-k before applying a filter can change the answer, so refusing
that rewrite is a correctness requirement.

## Architecture

```text
SQL + DataFusion optimizer --> VectorTable / VectorScanExec --> VectorIndex
                                                                  |
                                                        supplied FlatIndex
```

The DataFusion crate owns Arrow conversion, SQL-pattern matching, plan properties, limits, and output batches. The core
crate owns dimensions, metrics, exact ground truth, candidate selection, and deterministic result order. Later index
implementations will not import DataFusion.

This separation lets small Rust tests isolate the storage contract while SQLLogicTests verify the optimizer boundary.

## Contracts Established on Day 1

1. **Dimension:** a dataset has one nonzero dimension; every stored vector and query matches it.
2. **Numeric domain:** stored values are finite `f32`, while metric accumulation uses `f64`. Cosine inputs have nonzero
   norm.
3. **Identity:** core row offset `r` maps to Arrow batch row `r`, which carries the corresponding external ID and payload.
4. **Ordering:** lower internal distance is better. Ties use row offset. Dot product is negated at the metric boundary.
5. **Oracle:** exact search defines ground truth. Approximate latency is never reported without recall from the same data,
   queries, metric, and `k`.
6. **SQL safety:** the optimizer selects an index only when expression, metric, direction, dimension, and limit match its
   contract. Unsupported shapes remain exact.

## Course Progression

| Day | Estimate | Before | After |
| --- | ---: | --- | --- |
| [1 — DataFusion table and optimizer](./rust-02-datafusion.md) | 3–4 hours | Vectors are Rust structs and DataFusion has no table or vector access path. | Rows become an Arrow-backed `TableProvider`; exact top-k runs in DataFusion; a conservative sort-pushdown rule selects a compatible vector scan and preserves exact fallback. |
The ordering mirrors the maintained structure of the C++ course: establish representation and scan execution, then
implement safe index matching before an approximate index. The Rust course skips the C++ exact-executor chapter because
DataFusion already supplies vector expressions, bounded sort, and limit execution.

By the end, you should be able to explain:

- how row identity survives conversion from Rust structs to core offsets and Arrow arrays;
- which physical expression shapes are safe to lower to a vector index;
- why DataFusion retains exact fallback for filtered or incompatible top-k queries;
- why the optimizer rule must exist before an approximate index can be exercised from SQL.

## Deliberate Boundaries

Approximate indexes are later chapters. The Day 1 implementation also excludes online updates or deletes, index
persistence, crash recovery, concurrent mutation, filtered ANN, quantization, GPU kernels, distributed execution, DDL,
and a network service.

{{#include copyright.md}}
