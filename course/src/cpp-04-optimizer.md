# Matching a Vector Index

{{#include cpp-deprecation.md}}

This chapter replaces a safe exact top-k plan with `VectorIndexScanPlanNode`. The index implementations are still stubs,
so this checkpoint verifies plan matching and fallback behavior, not approximate-search results.

Complete [Exact K-Nearest Neighbors](./cpp-03-knn.md) first. You will likely modify:

```text
src/optimizer/vector_index_scan.cpp
src/optimizer/optimizer_custom_rules.cpp
```

*Related lecture:* [Query Planning & Optimization (CMU Intro to Database Systems)](https://www.youtube.com/watch?v=ePGPVJCyCAk&list=PLSE8ODhjZXjbj8BMuIrRcacnQh20hmY9g&index=15)

## Goal

Create a vector table and an HNSW index:

```sql
CREATE TABLE t1(v1 VECTOR(3), v2 integer);
CREATE INDEX t1v1hnsw ON t1 USING hnsw (v1 vector_l2_ops) WITH (m = 5, ef_construction = 64, ef_search = 10);
```

Your goal is to make compatible L2 top-k queries use this index while leaving unsafe or incompatible queries on the
exact path.

## Start from the Unoptimized Shape

The starter runs `OptimizeAsVectorIndexScan` before `OptimizeSortLimitAsTopN`. Keep that order for the default path
and match a `Limit` over `Sort`. After creating `t1` and a compatible vector index, run these statements directly in
`bustub-shell` to exercise the supported projections:

```sql
EXPLAIN (o) SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
EXPLAIN (o) SELECT * FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
EXPLAIN (o) SELECT v1, ARRAY [1.0, 1.0, 1.0] <-> v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
EXPLAIN (o) SELECT v2, v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
```

Before the vector-index rewrite, the rule sees these plan shapes.

**Case 1: `Sort` directly over `SeqScan`**

```text
Limit { limit=2 }
  Sort { order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")] }
    SeqScan { table=t1 }
```

Here `#0.0` directly names the vector column in the table schema.

**Case 2: `Sort` over a projected vector column**

```text
Limit { limit=2 }
  Sort { order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")] }
    Projection { exprs=["#0.0"] }
      SeqScan { table=t1 }
```

The sort expression names projection column `#0.0`, which maps to table column `#0.0`.

**Case 3: `Sort` over a projection that also computes distance**

```text
Limit { limit=2 }
  Sort { order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")] }
    Projection { exprs=["#0.0", "l2_dist([1.000000,1.000000,1.000000], #0.0)"] }
      SeqScan { table=t1 }
```

The projected distance does not change the lookup: the sort expression still reaches table column `#0.0` through the
projection.

**Case 4: `Sort` over reordered projected columns**

```text
Limit { limit=2 }
  Sort { order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.1)")] }
    Projection { exprs=["#0.1", "#0.0"] }
      SeqScan { table=t1 }
```

Here the sort expression names projection column `#0.1`, which maps back to table column `#0.0`. Do not assume that the
vector column is always the first projected column.

If no vector index matches, the later optimizer rule will still convert this pair to exact `TopN`. Moving the vector rule
after the Top-N rule and matching `TopN` instead is a valid extension, but do not try to support both shapes until the
default path works.

`VectorIndexScanExecutor` emits the table's original schema. If the matched plan contained a projection, clone that
projection above the new scan so the query still returns the same columns in the same order.

## Safe Match Contract

**Course rule:** Rewrite only when all of the following are true:

- the shape is the supported `Limit`/`Sort`/optional `Projection`/`SeqScan` chain;
- there is exactly one order-by expression and its direction is `Default` or ascending;
- the expression is a `VectorExpression` between a literal `ArrayExpression` and a table column;
- the selected index is a `VectorIndex` whose single key attribute is that table column;
- `VectorIndex::distance_fn_` matches the query's vector expression type; and
- the optional `vector_index_method` setting permits that index type.

The `VectorIndexScanPlanNode` stores an `ArrayExpression` as its base vector, so a column-to-column or other non-literal
query is outside this checkpoint. Filters, joins, multiple sort keys, descending distance, and unsupported plan shapes
must remain on the exact path. A fast plan that changes query meaning is a correctness bug.

**Prediction:** Suppose only a `vector_cosine_ops` index exists and the query orders by `<->` L2 distance. Should the
optimizer use the index? It must not: the ranking contract is different, so the exact `TopN` plan should remain.

## Index Selection Setting

`Optimizer::vector_index_match_method_` comes from `SET vector_index_method=...`:

- empty: accept a compatible IVFFlat or HNSW index;
- `hnsw`: accept only HNSW;
- `ivfflat`: accept only IVFFlat; and
- `none`: use exact search.

The catalog stores table indexes in an unordered map, so the particular compatible index chosen by the empty setting is
not a stable preference rule. Use `hnsw` or `ivfflat` when a deterministic choice matters.

## Verify the Checkpoint

From `bustub-vectordb/build`, run:

```shell
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/vector.03-index-selection.slt --verbose
```

The file uses `statement ok`, so inspect every `EXPLAIN` block. The positive cases should contain `VectorIndexScan`; after
`SET vector_index_method=none`, the plan should contain exact `TopN`.

<details>

<summary>Reference Test Result</summary>

```text
{{#include vector.03-index-selection.slt.ref}}
```

</details>

Add at least one negative manual case before moving on: use a metric mismatch, `ORDER BY ... DESC`, or an extra filter and
confirm that the plan stays exact. You are done when you can point to the comparison that checks the table column and the
comparison that checks `distance_fn_`, and explain what incorrect rows each prevents.

## Optional Extension

Support a plan that sorts by a projected distance alias:

```sql
EXPLAIN (o)
SELECT *
FROM (SELECT v1, ARRAY [1.0, 1.0, 1.0] <-> v1 AS distance FROM t1)
ORDER BY distance
LIMIT 2;
```

Before the Top-N rewrite, that query has this plan:

```text
Limit { limit=2 }
  Sort { order_bys=[("Default", "#0.1")] }
    Projection { exprs=["#0.0", "l2_dist([1.000000,1.000000,1.000000], #0.0)"] }
      SeqScan { table=t1 }
```

The sort expression is a column reference to the projection's computed distance. Trace it one additional step before
applying the same safety contract. This form reuses the projected distance instead of computing it again.

{{#include copyright.md}}
