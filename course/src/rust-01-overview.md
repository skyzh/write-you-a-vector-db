# Rust Course Preview

<div class="warning">

**Course status:** Exact search, recall evaluation, SQL index matching, and their focused tests are available as a preview.
The ANN implementations remain later cumulative chapters. Learner starter/completed refs and recorded human review remain
release requirements.

</div>

The Rust course builds exact search in a standalone crate, establishes the SQL index-matching boundary through DataFusion,
before implementing IVFFlat, NSW, and HNSW behind that boundary. The collection and search interfaces remain independently
testable without Arrow or a query engine, while SQLLogicTest will exercise every later index through the public SQL path.

## SQL Integration Boundary

[DataFusion](https://datafusion.apache.org/) provides SQL parsing, planning, Arrow execution, and the extension points used
by the course. It supports [`array_distance`, `cosine_distance`, and inner-product
functions](https://datafusion.apache.org/user-guide/sql/scalar_functions.html#array-functions). A custom
[`TableProvider`](https://datafusion.apache.org/library-user-guide/custom-table-providers.html) exposes the collection. The
design pins DataFusion 54.1.0 and uses `ExecutionPlan::try_pushdown_sort` and `with_fetch`: the physical optimizer offers
the scan a requested ordering, removes the generic sort after the scan accepts it, and passes the literal limit to the
index.

All DataFusion-specific code lives in the adapter crate. Students use the pinned public APIs and do not edit DataFusion
itself.

The course keeps returning to the same query:

```sql
SELECT id, payload
FROM points
ORDER BY cosine_distance(embedding, [0.1, 0.2, 0.3])
LIMIT 10;
```

The exact chapter first defines ground truth. The index-matching chapter then recognizes the compatible metric, constant
query vector, ordering direction, and literal limit and produces the course-defined `VectorIndexScanExec` backed by
`FlatIndex`. Each ANN chapter will change the selected index and add a SQLLogicTest without changing the SQL contract.
`EXPLAIN` makes the choice visible.

Queries outside the supported pattern retain the exact plan. In particular, the course does not claim that arbitrary
`WHERE` predicates can be applied after an ANN top-k without changing the result. Refusing an unsafe rewrite is part of
the SQL contract.

For example, suppose the nearest point belongs to tenant B and the second-nearest point belongs to tenant A. For
`WHERE tenant = 'A' ORDER BY distance LIMIT 1`, taking one ANN result and then applying the filter returns no rows. Applying
the filter first returns tenant A's point. The adapter must keep the exact plan for this query.

## Architecture

The dependency direction is:

```text
DataFusion SQL adapter --> collection API --> exact / IVF / graph index
                               ^
                               |
              unit tests, SQLLogicTest, benchmark
```

The adapter owns Arrow conversion, SQL-pattern recognition, plan properties, and result batches. The collection owns IDs,
dimensions, metrics, and index selection. The indexes know nothing about SQL, Arrow, or asynchronous execution.

This separation keeps every algorithm testable with ordinary Rust values. It also makes the final integration diff small
enough for students to explain line by line.

## System Contracts

The course establishes these contracts before students implement an ANN index.

1. A collection has one fixed dimension and one distance metric. A query or bulk-loaded point with another dimension is
   rejected.
2. Stored vectors use `f32`, while distance accumulation uses `f64`.
3. Exact search defines the ground truth. ANN benchmarks always report recall together with latency.
4. Results use deterministic tie-breaking so tests do not depend on heap or hash-map iteration order.
5. The required lifecycle is bulk load, build, freeze, and query. Persistence and online mutation after a build are out of
   scope.
6. SQL uses an ANN scan only when the adapter can prove that the query matches the index contract. All other queries use
   DataFusion's exact plan.

These rules are visible in public types, tests, and `EXPLAIN` output rather than scattered across chapter prose.

## Course Progression

The required path is sized for roughly one focused week, but it is not divided into artificial days. Learners may spread
it across more sessions. The chapters follow conceptual density: IVFFlat and the graph indexes are longer than the
baseline, evaluation, and adapter chapters.

| Chapter | Prerequisite | Initial estimate | Before | After |
| --- | --- | ---: | --- | --- |
| Exact search and ground truth | None | 2–3 hours | Vectors are ordinary arrays. | Dimensions and metrics have explicit semantics, and a bounded heap returns deterministic exact top-k results. |
| Benchmark and recall | Exact search and ground truth | 2–3 hours | Correctness examples are small and qualitative. | A seeded harness records exact ground truth, recall, p50/p99 latency, build time, and workload metadata. |
| Match a vector index from SQL | Benchmark and recall | 3–4 hours | Exact search is only a Rust API. | The adapter accepts a compatible sort as `VectorIndexScanExec`, preserves exact fallback, and establishes the SQLLogicTest ladder with `FlatIndex`. |
| IVFFlat | SQL index matching | 4–5 hours, likely two sessions | The matched scan still visits every vector. | Seeded k-means, inverted lists, and `probes` form a complete IVFFlat index measured through Rust and SQL tests. |
| NSW | IVFFlat checkpoint | 3–4 hours | Only partition-based ANN is available. | Greedy and beam search, incremental insertion, and neighbor pruning form a searchable single-layer graph exercised through SQL. |
| HNSW | NSW | 3–4 hours | Every graph search begins in the same layer. | Random levels, entry points, cross-layer descent, and `ef_search` form a hierarchical graph index. |

The ordering is intentional. Exact search becomes the oracle, and the benchmark comes before ANN so `probes`, beam width,
and `ef_search` are evaluated rather than guessed. SQL index matching comes next: using `FlatIndex` isolates planner and
fallback semantics from approximation and creates a SQLLogicTest harness before any ANN implementation needs it. IVFFlat,
NSW, and HNSW will then add one index and one SQL checkpoint apiece. HNSW follows NSW so hierarchy is the only new graph idea in
that chapter.

The ANN algorithms remain ordinary library code, but they will not be tested only through that library boundary. The
same SQL query and plan contract will accompany each implementation chapter, making integration failures visible at the commit
where the index is introduced.

Before finishing the course, students should be able to explain:

- why deterministic top-k ordering matters;
- why every ANN performance number needs a recall number from the same query set;
- how `probes` and `ef_search` change the candidate budget in different index families;
- which SQL expression shapes are safe to lower to an ANN scan; and
- why a filtered or differently ordered query must fall back to exact execution.

## What We Intentionally Leave Out

The required implementation does not include:

- online upserts and deletes after an index is built;
- persistent point or index formats, write-ahead logging, crash recovery, background rebuilds, or compaction;
- concurrent readers and writers or distributed execution;
- filtered ANN search, hybrid lexical search, or joins pushed into the vector index;
- quantization, GPU execution, or memory-mapped index layouts; or
- an HTTP or production-compatible database protocol.

These are follow-up projects, not hidden requirements. The final collection is useful for bulk-load-and-query workloads
and for demonstrating SQL integration, but it is not a production database.

{{#include copyright.md}}
