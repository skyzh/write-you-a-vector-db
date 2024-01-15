# Matching a Vector Index

Before building and using vector indexes, we will need to identify SQL queries that can be converted to an index scan, and use the index when it is possible to do so. In this task, you will implement the code to identify such queries and convert them into a vector index search plan node.

The list of files that you will likely need to modify:

```
src/optimizer/vector_index_scan.cpp
src/optimizer/optimizer_custom_rules.cpp
```

*Related Lectures*

* [Query Planning & Optimization (CMU Intro to Database Systems)](https://www.youtube.com/watch?v=ePGPVJCyCAk&list=PLSE8ODhjZXjbj8BMuIrRcacnQh20hmY9g&index=15)

## Matching the Plan Structure

```sql
CREATE TABLE t1(v1 VECTOR(3), v2 integer);
CREATE INDEX t1v1hnsw ON t1 USING hnsw (v1 vector_l2_ops) WITH (m = 5, ef_construction = 64, ef_search = 10);
```

You will need to convert queries like below to a vector index scan.

```sql
EXPLAIN (o) SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
EXPLAIN (o) SELECT * FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
EXPLAIN (o) SELECT v1, ARRAY [1.0, 1.0, 1.0] <-> v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
EXPLAIN (o) SELECT v2, v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 2;
```

...where the distance expressions appear in the order-by expressions.

You may choose to run the conversion rule before or after the top-n optimization rule. If you choose to invoke this rule before converting to top-n, you will need to match sort and limit plan nodes. Otherwise, you may want to match the top-n plan node. The rule order can be found in `optimizer_custom_rules.cpp`.

There are three cases you should consider:

**Case 1: TopN directly followed by SeqScan**

```
TopN { n=2, order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")]}
  SeqScan { table=t1 }
```

**Case 2: TopN followed by Projection**

```
TopN { n=2, order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")]}
  Projection { exprs=["#0.0", "l2_dist([1.000000,1.000000,1.000000], #0.0)"] }
    SeqScan { table=t1 }
```

**Case 3: TopN followed by Projection with column reference shuffled**

```
TopN { n=2, order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.1)")]}
  Projection { exprs=["#0.1", "#0.0"] }
    SeqScan { table=t1 }
```

As a rule of thumb, you should always figure out which vector column does the user wants to compare with. In all cases, it will be the first column of the sequential scan, that is to say, the first column in the vector table.

In case 1, as the top-n node directly references the first column `#0.0`, you can easily retrieve the column.

In case 2, the top-n node references `#0.0`, which is the first column of the projection node, and the first column of the projection column is `#0.0`.

In case 3, the top-n node references `#0.1`, which is the second column of the projection node, which eventually references `#0.0` of the sequential scan executor.

After identifying the index column, you may iterate the catalog and find the first available index to use. Then, you may replace the plan with a vector index scan plan node.

The vector index scan plan node will emit the table tuple in its original schema order. Therefore, in case 2 and 3, you will also need to add a projection after the vector index scan plan node in order to keep the semantics of the query. 

## Index Selection Strategy

In the `Optimizer` class, you may make use of the `vector_index_match_method_` member variable to decide the index selection strategy.

* `unset` or empty: match the first vector index available.
* `hnsw`: only match the HNSW index.
* `ivfflat`: only match the IVFFlat index.
* `none`: do not match any index and do exact nearest-neighbor search.

## Testing

You can run the test cases using SQLLogicTest.

```
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/vector.03-index-selection.slt --verbose
```

The test cases do not do any correctness checks and you will need to compare with the below output by yourself.

<details>

<summary>Reference Test Result</summary>

```
{{#include vector.03-index-selection.slt.ref}}
```

</details>

## Bonus Tasks

**Optimize more queries**

There are many other queries that can be converted to a vector index scan. You extend the range of queries that can be converted to a vector index scan in your optimizer rule. For example,


```sql
EXPLAIN (o) SELECT * FROM (SELECT v1, ARRAY [1.0, 1.0, 1.0] <-> v1 as distance FROM t1) ORDER BY distance LIMIT 2;
```

The query plan for this query is:

```
TopN { n=2, order_bys=[("Default", "#0.1")]}
  Projection { exprs=["#0.0", "l2_dist([1.000000,1.000000,1.000000], #0.0)"] }
    SeqScan { table=t1 }
```

This query saves one extra computation of the distance compared with the one in the sqllogictest. You may modify your optimizer rule to convert this query into a vector index scan.
