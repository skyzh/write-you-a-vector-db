# Exact K-Nearest Neighbors

{{#include cpp-deprecation.md}}

This chapter turns an ordinary table scan into exact k-nearest-neighbor search. First you will implement general-purpose
sort and limit executors. Then you will replace that pair with a bounded Top-N executor.

Complete [Vector Expressions and Storage](./cpp-02-setup.md) first. You will likely modify these private BusTub assignment
files:

```text
src/execution/sort_executor.cpp                      (KEEP PRIVATE)
src/execution/topn_executor.cpp                      (KEEP PRIVATE)
src/execution/limit_executor.cpp                     (KEEP PRIVATE)
src/include/execution/executors/sort_executor.h      (KEEP PRIVATE)
src/include/execution/executors/topn_executor.h      (KEEP PRIVATE)
src/include/execution/executors/limit_executor.h     (KEEP PRIVATE)
src/optimizer/sort_limit_as_topn.cpp                 (KEEP PRIVATE)
```

<div class="warning">

These files overlap with CMU's Database Systems assignments. **KEEP PRIVATE** means that you must add these paths to your
solution repository's `.gitignore` and must not commit or publish them. The starter already tracks placeholder versions,
so adding them to `.gitignore` inside the starter clone is not enough to hide your changes. Keep the entire clone private.

</div>

## The Query

```sql
CREATE TABLE t1(v1 VECTOR(3), v2 integer);
SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 3;
```

The query scans every row, computes its distance from `[1, 1, 1]`, and returns the three smallest distances. Before the
Top-N rewrite, run the following statement in `bustub-shell`:

```sql
EXPLAIN (o) SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 3;
```

Its plan has this shape:

```text
Limit { limit=3 }
  Sort { order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")] }
    Projection { exprs=["#0.0"] }
      SeqScan { table=t1 }
```

`#0.0` means column 0 from child 0. Evaluate each order-by expression against the child tuple and the child's output
schema.

## Checkpoint 1: Sort and Limit

The sort executor is a pipeline breaker: `Init` consumes and stores every `(Tuple, RID)` from its child, then sorts the
stored entries. `Next` emits them one at a time. Keep the RID paired with its tuple throughout the sort.

**Course rules:**

- Compare order-by expressions from left to right. Continue to the next expression only when the current values tie.
- `Default` has ascending behavior. Support both explicit ascending and descending order.
- The required tests use non-null sort keys. If you add null support, define its placement consistently.
- Preserve any order among complete ties; the vector reference allows tied rows to appear in either order.

The limit executor initializes its child and forwards at most `limit` entries. It must handle `limit = 0` and a child with
fewer rows without pulling or emitting an extra tuple.

From `bustub-vectordb/build`, run:

```shell
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/p3.16-sort-limit.slt
./bin/bustub-sqllogictest ../test/sql/vector.02-naive-knn.slt --verbose
```

The first file checks multi-column ascending and descending order. The vector file exercises all three distance functions;
compare its exact-query rows with the first reference output.

<details>

<summary>Sort + Limit Reference</summary>

```text
{{#include vector.02-naive-knn.slt.1.ref}}
```

</details>

## Checkpoint 2: Bounded Top-N

Sorting all `n` rows costs `O(n log n)` and stores all `n` entries. For `LIMIT k`, a max-heap can retain only the best `k`
entries in `O(n log k)` time and `O(k)` space.

First implement `OptimizeSortLimitAsTopN`. It should replace only a `Limit` whose direct child is a `Sort`, copy the sort's
order-by list and the limit into a `TopNPlanNode`, and preserve the sort's child.

Then implement `TopNExecutor`:

1. initialize the child;
2. evaluate the same full ordering used by `SortExecutor`;
3. keep at most `k` best `(Tuple, RID)` entries in a max-heap, with the worst retained entry at the top; and
4. emit the retained entries in final best-to-worst order.

Popping a max-heap directly produces the worst retained row first. Reverse that sequence, or use another equivalent
method, before `Next` begins emitting. `GetNumInHeap()` must report the bounded container's size because the focused test
uses it to catch implementations that secretly store the whole input.

**Prediction:** If the input distances are `4, 1, 3, 2` and `k = 2`, which values remain after each input? The final output
must be `1, 2`, even though the heap's top is `2`.

Run:

```shell
./bin/bustub-sqllogictest ../test/sql/p3.17-topn.slt
./bin/bustub-sqllogictest ../test/sql/vector.02-naive-knn.slt --verbose
```

The `EXPLAIN` output should now contain `TopN` instead of `Limit` over `Sort`, and its query rows should match the exact
checkpoint apart from allowed tie ordering.

<details>

<summary>Top-N Reference</summary>

```text
{{#include vector.02-naive-knn.slt.2.ref}}
```

</details>

You are done when you can explain why changing the Top-N heap from a max-heap to a min-heap would retain the wrong end of
the ordering, and how the optimizer rewrite preserves the original plan's result.

*Related lecture:* [Query Planning & Optimization (CMU Intro to Database Systems)](https://www.youtube.com/watch?v=ePGPVJCyCAk&list=PLSE8ODhjZXjbj8BMuIrRcacnQh20hmY9g&index=15)

## Optional Extension

Extend vector construction to accept mixed integer and decimal array literals, or a cast such as
`'[1.0, 1.0, 1.0]'::VECTOR(3)`.

{{#include copyright.md}}
