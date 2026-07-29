# Build Vector Search in Rust

<div class="warning">

**Course status:** Days 1–2 are ready to learn from and implement. The repository includes learner starter code, focused
tests, and separate reference solutions.

</div>

In two days, you will connect an in-memory vector table to DataFusion, implement the optimizer rule that selects a safe
vector-index scan, and build IVFFlat behind that rule. The integration day comes first so the algorithm can be tested
through SQL as soon as it works.

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

The Cargo workspace under `rust/` separates learner and reference trees:

```text
vector-starter/
  core/                      validated dataset and IVFFlat TODOs
  datafusion/                Day 1 Arrow table and optimizer-rule TODOs
vector/
  core/                      completed core reference
  datafusion/                completed DataFusion reference
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

The supplied flat index lets you verify this rule before IVFFlat exists. On Day 2, the plan changes only at the index:

```text
SortExec: TopK(fetch=10), ...
  VectorIndexScanExec: index=ivf_flat, metric=Cosine, query_dim=3, fetch=Some(10), ordered=false
```

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
                                                                  |
                                                       learner IvfFlatIndex
```

The DataFusion crate owns Arrow conversion, SQL-pattern matching, plan properties, limits, and output batches. The core
crate owns dimensions, metrics, exact ground truth, candidate selection, and deterministic result order. Later index
implementations will not import DataFusion.

This separation gives Day 2 two useful views of the same checkpoint: small Rust tests isolate the algorithm, while an
SQLLogicTest proves the Day 1 optimizer can reach it.

## Contracts That Survive Both Days

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
| [2 — IVFFlat](./rust-03-ivfflat.md) | 4–5 hours | The optimizer can select only the supplied flat test index. | Seeded k-means, inverted lists, and `probes` create a measured recall/work tradeoff behind the same SQL query. |

The ordering mirrors the maintained structure of the C++ course: establish representation and scan execution, then
implement safe index matching, then implement the index. The Rust course skips the C++ exact-executor chapter because
DataFusion already supplies vector expressions, bounded sort, and limit execution.

By the end, you should be able to explain:

- how row identity survives conversion from Rust structs to core offsets and Arrow arrays;
- which physical expression shapes are safe to lower to a vector index;
- why DataFusion retains exact fallback for filtered or incompatible top-k queries;
- why the optimizer rule must exist before an approximate index can be exercised from SQL;
- why IVFFlat must rebuild list membership after its final centroid update; and
- how `probes` trades candidate work for recall without changing SQL.

## Deliberate Boundaries

The published path stops after IVFFlat. Graph indexes are later work. The implementation also excludes online updates or
deletes, index persistence, crash recovery, concurrent mutation, filtered ANN, quantization, GPU kernels, distributed
execution, DDL, and a network service.

{{#include copyright.md}}
