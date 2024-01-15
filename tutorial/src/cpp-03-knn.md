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

**WARNING:** Some part of this chapter overlaps with the CMU-DB's Database System course and we ask you NOT to put the above files in a public repo.

## Naive K-Nearest Neighbors

In vector databases, one of the most important operations is to find nearest neighbors to a user-provided vector in the vector table using a specified vector distance function. In this task, you will need to implement some query executors in order to support nearest neighbor SQL queries as below:

```sql
CREATE TABLE t1(v1 VECTOR(3), v2 integer);
SELECT v1 FROM t1 ORDER BY ARRAY [1.0, 1.0, 1.0] <-> v1 LIMIT 3;
```

The query scans the table, computes the distances between vectors in the table and `<1.0, 1.0, 1.0>`, and returns 3 nearest neighbors to the query vector.

## Sort and Limit Executor

Explain the query.

Sort and limit.

std::sort with lambda function.

## Top-n Optimization

You do not need to sort the entire dataset. You may use a binary heap (`priority_queue` in C++ STL) to compute the same result set with more efficiency.

The first step is to write an optimizer rule to convert sort and limit into a top-n executor.

Then, implement the top-n executor.

## Test Cases

## Bonus Tasks

Now that you have a better view of how BusTub works, you may choose to complete the below bonus tasks to enhance your understanding and challenge yourself.

**Construct vectors from string**

Currently, the query processing layer only supports creating a vector from array keyword and a list of decimal values like `SELECT ARRAY [1.0, 1.0, 1.0]`. You may extend the syntax to support (1) create a vector from integers `SELECT ARRAY [1, 1.0, 1]` and (2) create a vector from string `SELECT '[1.0, 1.0, 1.0]'::VECTOR(3)`.

*Again, please keep your implementation in this section private and do not put them in a public repo because they overlap with the CMU-DB's Database Systems course projects.*
