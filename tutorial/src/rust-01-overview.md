# 🚧 The Rust Course

> 🚧 **Course status:** The cumulative reference implementation, focused tests, and executable chapters are available as
> a preview. Learner starter/completed refs and recorded human review remain release requirements.

The Rust course builds vector indexes in a standalone crate, then connects them to SQL through DataFusion in the final
required chapter. The same collection and search interfaces remain independently testable without Arrow or a query
engine. At the boundary, a small adapter makes the relationship between SQL semantics and index execution visible.

## SQL Integration Boundary

[DataFusion](https://datafusion.apache.org/) provides SQL parsing, planning, Arrow execution, and the extension points used
by the course. It already supports [`array_distance`, `cosine_distance`, and inner-product
functions](https://datafusion.apache.org/user-guide/sql/scalar_functions.html#array-functions). A custom
[`TableProvider`](https://datafusion.apache.org/library-user-guide/custom-table-providers.html) exposes the collection. The
preview pins DataFusion 54.1.0 and uses its public `ExecutionPlan::try_pushdown_sort` and `with_fetch` extension points: the
built-in physical optimizer offers the scan a requested ordering, removes the generic sort after the scan accepts it, and
passes the literal limit to the index.

All DataFusion-specific code lives in the adapter crate. Students use the pinned public APIs and do not edit DataFusion
itself.

The course begins and ends with the same query:

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [0.1, 0.2, 0.3])
LIMIT 10;
```

In the opening chapter, DataFusion evaluates the distance for every row and performs a generic top-k sort. In the final
chapter, the adapter recognizes the compatible metric, constant query vector, ordering direction, and literal limit, then
produces the course-defined `VectorIndexScanExec`. `EXPLAIN` makes the change visible.

Queries outside the supported pattern retain the exact plan. In particular, the short course does not claim that
arbitrary `WHERE` predicates can be applied after an ANN top-k without changing the result. Refusing an unsafe rewrite is
part of demonstrating that the integration is intuitive rather than magical.

For example, suppose the nearest point belongs to tenant B and the second-nearest point belongs to tenant A. For
`WHERE tenant = 'A' ORDER BY distance LIMIT 1`, taking one ANN result and then applying the filter returns no rows. Applying
the filter first returns tenant A's point. The required adapter must keep the exact plan for this query.

## Architecture

The dependency direction is:

```text
DataFusion SQL adapter --> collection API --> exact / IVF / graph index
                               ^
                               |
                    tests, datasets, benchmark
```

The adapter owns Arrow conversion, SQL-pattern recognition, plan properties, and result batches. The collection owns IDs,
dimensions, metrics, and index selection. The indexes know nothing about SQL, Arrow, or asynchronous execution.

This separation keeps every algorithm testable with ordinary Rust values. It also makes the final integration diff small
enough for students to explain line by line.

## System Contracts

The course architecture establishes these contracts before students implement an ANN index.

1. A collection has one fixed dimension and one distance metric. A query or bulk-loaded point with another dimension is
   rejected.
2. Stored vectors use `f32`. Distance accumulation may use a wider type when a chapter demonstrates why it matters.
3. Exact search defines the ground truth. ANN benchmarks always report recall together with latency.
4. Results use deterministic tie-breaking so tests do not depend on heap or hash-map iteration order.
5. The required lifecycle is bulk load, build, freeze, and query. Persistence and online mutation after a build are out of
   scope.
6. SQL uses an ANN scan only when the adapter can prove that the query matches the index contract. All other queries use
   DataFusion's exact plan.

These rules are visible in public types, tests, and `EXPLAIN` output rather than scattered across chapter prose.

## Short Course Structure

The required path is intended to fit roughly one focused week, but it is not divided into artificial days. Learners may
spread it across more sessions, and final estimates will be calibrated against the unpublished starter diffs before
release.

The topics below are separate chapters because each produces a coherent, reviewable checkpoint. Their time budgets reflect
content density: IVFFlat and the graph indexes are longer than the baseline, evaluation, and adapter chapters.

| Chapter | Initial estimate | Before | After |
| --- | ---: | --- | --- |
| Exact search and the SQL baseline | 2–3 hours | Vectors are ordinary arrays, and the target SQL query has no index. | Dimensions and metrics have explicit semantics, a bounded heap returns deterministic exact top-k results, and `EXPLAIN` records the exhaustive DataFusion plan. |
| Benchmark and recall | 2–3 hours | Correctness examples are small and qualitative. | A seeded harness records exact ground truth, recall, p50/p99 latency, build time, and workload metadata. |
| IVFFlat | 4–5 hours, likely two sessions | Exact search visits every vector. | Seeded k-means, inverted lists, and `probes` form a complete IVFFlat index with a measured recall/latency curve. |
| NSW | 3–4 hours | Only partition-based ANN is available. | Greedy and beam search, incremental insertion, and neighbor pruning form a searchable single-layer graph. |
| HNSW | 3–4 hours | Every graph search begins in the same layer. | Random levels, entry points, cross-layer descent, and `ef_search` form a hierarchical graph index. |
| Use the index from SQL | 3–4 hours | DataFusion still evaluates every distance and performs a generic top-k. | The adapter accepts the compatible sort as `VectorIndexScanExec`, preserves exact fallback, and compares both plans on the same workload. |

The ordering is intentional. Exact search becomes the first working implementation and the oracle for every approximate
index. The benchmark comes before ANN so `probes`, beam width, and `ef_search` are evaluated rather than guessed. IVFFlat
introduces the recall/latency tradeoff with a simple candidate-generation model. HNSW follows NSW so hierarchy is the only
new graph idea in that chapter. SQL integration comes last so it wraps an already tested search contract.

SQL appears in the opening and final chapters rather than consuming the middle of the course. Students know the target plan
from the beginning, but the ANN algorithms remain ordinary library code. The final chapter demonstrates how little adapter
code is needed and how much semantic care that small amount of code still requires.

Before finishing the course, students should be able to explain:

- why deterministic top-k ordering matters;
- why every ANN performance number needs a recall number from the same query set;
- how `probes` and `ef_search` change the candidate budget in different index families;
- which SQL expression shapes are safe to lower to an ANN scan; and
- why a filtered or differently ordered query must fall back to exact execution in the required implementation.

## What We Intentionally Leave Out

A short course needs a hard boundary. The required implementation does not include:

- online upserts and deletes after an index is built;
- persistent point or index formats, write-ahead logging, crash recovery, background rebuilds, or compaction;
- concurrent readers and writers or distributed execution;
- filtered ANN search, hybrid lexical search, or joins pushed into the vector index;
- quantization, GPU execution, or memory-mapped index layouts; or
- an HTTP or production-compatible database protocol.

These are follow-up projects, not hidden requirements. The final collection is useful for bulk-load-and-query workloads and
for demonstrating SQL integration, but it is not a production database.

{{#include copyright.md}}
