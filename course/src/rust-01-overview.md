# Build Vector Search in Rust

<div class="warning">

**Course status:** Chapters 1–3 are ready to implement. The repository includes starter code, focused tests, and separate
reference solutions.

</div>

In the first three chapters, you will connect an in-memory vector table to DataFusion, implement the optimizer rule that
selects a safe vector-index scan, build IVFFlat behind that rule, and then navigate a proximity graph with NSW. Every
chapter ends with a runnable SQL query, so you can inspect how the physical plan changes as the index becomes more capable.

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [0.1, 0.2, 0.3])
LIMIT 10;
```

Your first SQL query uses DataFusion's vector distance expressions, bounded sort, and `LIMIT` to return an exact result.
The starter includes a `FlatIndex`, which checks every vector, while you connect the table to the query planner. You will
then add IVFFlat as your own candidate selector behind the same query.

## Choose Your Workspace

The Cargo workspace under `rust/` separates starter and reference trees:

```text
vector-starter/
  core/                      dataset, IVFFlat, and NSW TODOs
  datafusion/                Chapter 1 Arrow table and optimizer-rule TODOs
vector/
  core/                      completed core reference
  datafusion/                completed DataFusion reference
```

Work in `vector-starter/` and implement its TODOs in chapter order. The `vector/` tree contains completed references; keep
it closed while you work through the exercises, as required by the starter's `AGENTS.md` files.

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

In Chapter 1, you implement `ExecutionPlan::try_pushdown_sort`. It accepts only one compatible distance ordering over the
`embedding` column with a literal query vector. `with_fetch` receives `LIMIT k`, and the matched scan asks the selected
index for `k` candidate row offsets:

```text
SortExec: TopK(fetch=10), ...
  VectorIndexScanExec: index=flat, metric=Cosine, query_dim=3, fetch=Some(10), ordered=false
```

The starter's exact `FlatIndex` lets you exercise this rule in Chapter 1. Later chapters change only the selected index:

```text
SortExec: TopK(fetch=10), ...
  VectorIndexScanExec: index=ivf_flat, metric=Cosine, query_dim=3, fetch=Some(10), ordered=false
```

```text
SortExec: TopK(fetch=10), ...
  VectorIndexScanExec: index=nsw, metric=Cosine, query_dim=3, fetch=Some(10), ordered=false
```

The default plan retains DataFusion's bounded sort. The index selects candidates; `SortExec` owns SQL ordering. When the
selected index returns rows in the requested order, `SET vector_search.ordered = true` tells DataFusion it can skip this
final sort.

Filters, multiple sort keys, a non-literal query vector, the wrong distance function, the wrong direction, or a dimension
mismatch keep the exact plan. In particular, taking ANN top-k before applying a filter can change the answer, so refusing
that rewrite is a correctness requirement.

## Architecture

```text
SQL + DataFusion optimizer --> VectorTable / VectorScanExec --> VectorIndex
                                                                  |
                                                        exact FlatIndex
                                                                  |
                                                       your IvfFlatIndex
                                                                  |
                                                          your NswIndex
```

The DataFusion crate owns Arrow conversion, SQL-pattern matching, plan properties, limits, and output batches. The core
crate owns dimensions, metrics, exact-search results, candidate selection, and deterministic result order. Later index
implementations will not import DataFusion.

This separation gives each index chapter two useful views of the same checkpoint: small Rust tests isolate the algorithm,
while an SQLLogicTest shows that the Chapter 1 optimizer can reach it.

## System Contract

1. **Dimension:** a dataset has one nonzero dimension; every stored vector and query matches it.
2. **Numeric domain:** stored values are finite `f32`, while metric accumulation uses `f64`. Cosine inputs have nonzero
   norm.
3. **Identity:** core row offset `r` maps to Arrow batch row `r`, which carries the corresponding external ID and payload.
4. **Ordering:** lower internal distance is better. Ties use row offset. Dot product is negated at the metric boundary.
5. **Exact baseline:** exact search defines the expected result. When you report approximate latency, include recall from
   the same data, queries, metric, and `k`.
6. **SQL safety:** the optimizer selects an index only when expression, metric, direction, dimension, and limit match its
   contract. Unsupported shapes remain exact.

## Course Progression

| Chapter | Estimate | Before | After |
| --- | ---: | --- | --- |
| [1 — DataFusion table and optimizer](./rust-02-datafusion.md) | 3–4 hours | Vectors are Rust structs and DataFusion has no table or vector access path. | Rows become an Arrow-backed `TableProvider`; exact top-k runs in DataFusion; a conservative sort-pushdown rule selects a compatible vector scan and preserves exact fallback. |
| [2 — IVFFlat](./rust-03-ivfflat.md) | 4–5 hours | A flat index handles matched SQL top-k queries exactly. | Seeded k-means, inverted lists, and `probes` create a measured recall/work tradeoff behind the same SQL query. |
| [3 — NSW](./rust-04-nsw.md) | 4–5 hours | Candidate selection comes from centroid partitions. | Best-first traversal and bounded reciprocal graph insertion expose `ef_search` as a second recall/work tradeoff behind the same SQL query. |

Chapter 1 gives you an exact end-to-end query whose rows and physical plan you can inspect. Chapters 2 and 3 keep that SQL
interface and safety rule in place while changing how candidate rows are selected.

After Chapter 3, you should be able to explain:

- how row identity survives conversion from Rust structs to core offsets and Arrow arrays;
- which physical expression shapes are safe to lower to a vector index;
- why DataFusion retains exact fallback for filtered or incompatible top-k queries;
- why the optimizer rule must exist before an approximate index can be exercised from SQL;
- why IVFFlat must rebuild list membership after its final centroid update; and
- how `probes` trades candidate work for recall without changing SQL;
- why NSW needs separate candidate and result frontiers; and
- how reciprocal pruning preserves a bounded graph.

## Scope

These chapters use an immutable in-memory collection. The next chapter adds hierarchy to NSW. Online updates or deletes,
index persistence, crash recovery, concurrent mutation, filtered ANN, quantization, GPU kernels, distributed execution,
DDL, and a network service remain outside this implementation.

{{#include copyright.md}}
