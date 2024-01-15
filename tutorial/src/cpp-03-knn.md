# Naive K-Nearest Neighbors

In this task, we will implement a naive k-nearest neighbor search by simply scanning the table, computing the distance, and retrieving the k-nearest elements.

The list of files that you will likely need to modify:

```
src/execution/sort_executor.cpp                      (KEEP PRIVATE)
src/execution/topn_executor.cpp                      (KEEP PRIVATE)
src/execution/limit_executor.cpp                     (KEEP PRIVATE)
src/include/execution/executors/sort_executor.h      (KEEP PRIVATE)
src/include/execution/executors/topn_executor.h      (KEEP PRIVATE)
src/include/execution/executors/limit_executor.h     (KEEP PRIVATE)
src/optimizer/sort_limit_as_topn.cpp                 (KEEP PRIVATE)
```

<div class="warning">

**WARNING:** Some part of this chapter overlaps with the CMU-DB's Database System course and we ask you NOT to put the above files in a public repo.

</div>

## Naive K-Nearest Neighbors

In vector databases, one of the most important operations is to find nearest neighbors to a user-provided vector in the vector table using a specified vector distance function. In this task, you will need to implement some query executors in order to support nearest neighbor SQL queries as below:

```sql
CREATE TABLE t1(v1 VECTOR(3), v2 integer);
SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 3;
```

The query scans the table, computes the distances between vectors in the table and `<1.0, 1.0, 1.0>`, and returns 3 nearest neighbors to the query vector. Explain the query, and you will see the query plan as below.

```
bustub> explain (o) SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 3;
=== OPTIMIZER ===
Limit { limit=3 }
  Sort { order_bys=[("Default", "l2_dist([1.000000,1.000000,1.000000], #0.0)")] }
    Projection { exprs=["#0.0"] }
      SeqScan { table=t1 }
```

BusTub uses limit and sort executor to process this SQL query. In this part, you will need to implement these two executors, and optimize them into a top-k executor which computes nearest-k neighbors more efficiently.


## Sort and Limit Executor

The sort executor pulls all the data from the child executor, sort them in memory, and emit the data the the parent executor. You should order the data as indicated in the query plan. In the above example, the query plan indicates the data to be ordered by `l2_dist([1.000000,1.000000,1.000000], #0.0)` in the default (ascending) order. `#0.0` is a column value expression which returns the *first* column in the *first* child executor. You may use `Evaluate` on an expression to retrieve the distance to be ordered.

After getting all the data and RIDs from the child executor in sort executor's `Init` function, you can use `std::sort` to sort the tuples. The comparison function should be implemented as a for loop over the query plan's order-by requirement. You can then implement sort executor's `Next` function as emitting sorted tuples one by one.

Limit executor returns the first `limit` number of elements from the child executor. You can get all necessary information in the query plan, and stop getting data from the child executor and emitting to the parent executor when the limit is reached.

After implementing these two executors, you should be able to get k-nearest neighbors of the base vector in BusTub.

## Testing Sort + Limit

At this point, you can run the test cases using SQLLogicTest.

```
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/vector.02-naive-knn.slt --verbose
```

The test cases do not do any correctness checks and you will need to compare with the below output by yourself. Your result could be different from the reference solution because your way of breaking the tie (i.e., when two distances are the same) might be different.


<details>

<summary>Reference Test Result</summary>

```
{{#include vector.02-naive-knn.slt.1.ref}}
```

</details>


## Top-N Optimization

To retrieve the k-nearest neighbor, you do not need to sort the entire dataset. You may use a binary heap (`priority_queue` in C++ STL) to compute the same result set with more efficiency. This requires you to combine sort and limit executor into a single top-n executor.

The first step is to write an optimizer rule to convert sort and limit into a top-n executor. You will need to match a limit plan node with a sort child plan node, get necessary information (order-bys and limit), and then create a top-n plan node. There are already some example optimizer rule implementaions and you may refer to them.

Then, you may implement the top-n executor. The logic is similar to the sort executor that you do all the computation work in the `Init` function and then emit the top-k tuples in the `Next` function one by one. You will need to maintain a max-heap that contains the minimum k elements when scanning from the child executor.

*Related Lectures*

* [Query Planning & Optimization (CMU Intro to Database Systems)](https://www.youtube.com/watch?v=ePGPVJCyCAk&list=PLSE8ODhjZXjbj8BMuIrRcacnQh20hmY9g&index=15)

## Testing TopN

At this point, you can run the test cases using SQLLogicTest.

```
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/vector.02-naive-knn.slt --verbose
```

The test cases do not do any correctness checks and you will need to compare with the below output by yourself. Your result could be different from the reference solution because your way of breaking the tie (i.e., when two distances are the same) might be different. Note that you should see `TopN` instead of sort and limit plan nodes in your explain result.

<details>

<summary>Reference Test Result</summary>

```
{{#include vector.02-naive-knn.slt.2.ref}}
```

</details>

## Bonus Tasks

Now that you have a better view of how BusTub works, you may choose to complete the below bonus tasks to enhance your understanding and challenge yourself.

**Construct vectors from string**

Currently, the query processing layer only supports creating a vector from array keyword and a list of decimal values like `SELECT ARRAY [1.0, 1.0, 1.0]`. You may extend the syntax to support (1) create a vector from integers `SELECT ARRAY [1, 1.0, 1]` and (2) create a vector from string `SELECT '[1.0, 1.0, 1.0]'::VECTOR(3)`.

*Again, please keep your implementation in this section private and do not put them in a public repo because they overlap with the CMU-DB's Database Systems course projects.*
