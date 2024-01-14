# Vector Storage and Expressions

In this chapter, we will walk through some ramp-up tasks to get familiar with the BusTub system. You will be able to store vectors inside the system and compute vector distances after finishing all required tasks.

The list of files that you will likely need to modify:

```
src/execution/insert_executor.cpp                    (recommended to git-ignore)
src/execution/seq_scan_executor.cpp                  (recommended to git-ignore)
src/include/execution/executors/insert_executor.h    (recommended to git-ignore)
src/include/execution/executors/seq_scan_executor.h  (recommended to git-ignore)
src/include/execution/expressions/vector_expression.h
```

**WARNING:** In this tutorial, you will implement a simplified version of the sequential scan and the insert executor. These implementations are different from the 15-445 course but we still recommend you not including these files in your git repository.

## Computing Distances

In `vector_expressions.h`, you can implement some distance functions that we will use when building vector indexes and finding the k-nearest neighbors. You will need to implement 3 distance functions in `ComputeDistance`.

**L2 distance (or Euclidean distance)**

\\( = \lVert \mathbf{a} - \mathbf{b} \rVert = \sqrt {(a_1 - b_1)^2 + (a_2 - b_2)^2 + \cdots + (a_n - b_n)^2} \\)

**Cosine Similarity Distance** 

\\( = 1 - \frac { \mathbf{a} \cdot \mathbf{b} } {\lVert \mathbf{a} \rVert \lVert \mathbf{b} \rVert} = 1 - \frac { a_1 b_1 + a_2 b_2 + \cdots + a_n b_n} { \sqrt { a_1^2 + \cdots + a_n^2 } \sqrt { b_1^2 + \cdots + b_n^2 } } \\)

**Negative Inner Product Distance**

\\( = - \mathbf{a} \cdot \mathbf{b} = - ( a_1 b_1 + a_2 b_2 + \cdots + a_n b_n ) \\)

## Insertion and Sequential Scan

In this task, you will learn how BusTub represents data and how to interact with vector indexes.

### Table Heap and Vector Indexes

### Insertion

### Sequential Scan

## Test Cases

You can run the test cases using SQLLogicTest.

```
./bin/bustub-sqllogictest ../test/sql/vector.01-insert-scan.slt --verbose
```

The test cases do not do any correctness checks and you will need to compare with the below output by yourself.

<details>

<summary>Reference Test Result</summary>

```
<main>:2
SELECT ARRAY [1.0, 1.0, 1.0] <-> ARRAY [-1.0, -1.0, -1.0] as distance;
----
3.464102	

<main>:5
SELECT ARRAY [1.0, 1.0, 1.0] <=> ARRAY [-1.0, -1.0, -1.0] as distance;
----
2.000000	

<main>:8
SELECT inner_product(ARRAY [1.0, 1.0, 1.0], ARRAY [-1.0, -1.0, -1.0]) as distance;
----
3.000000	

<main>:11
CREATE TABLE t1(v1 VECTOR(3), v2 integer);
----
Table created with id = 24	

<main>:14
INSERT INTO t1 VALUES (ARRAY [1.0, 1.0, 1.0], 1), (ARRAY [2.0, 1.0, 1.0], 2), (ARRAY [3.0, 1.0, 1.0], 3), (ARRAY [4.0, 1.0, 1.0], 4);
----
0	

<main>:17
SELECT * FROM t1;
----
[1,1,1]	1	
[2,1,1]	2	
[3,1,1]	3	
[4,1,1]	4	

<main>:20
SELECT v1, ARRAY [1.0, 1.0, 1.0] <-> v1 as distance FROM t1;
----
[1,1,1]	0.000000	
[2,1,1]	1.000000	
[3,1,1]	2.000000	
[4,1,1]	3.000000	

<main>:23
SELECT v1, ARRAY [1.0, 1.0, 1.0] <=> v1 as distance FROM t1;
----
[1,1,1]	0.000000	
[2,1,1]	0.057191	
[3,1,1]	0.129612	
[4,1,1]	0.183503	

<main>:26
SELECT v1, inner_product(ARRAY [1.0, 1.0, 1.0], v1) as distance FROM t1;
----
[1,1,1]	-3.000000	
[2,1,1]	-4.000000	
[3,1,1]	-5.000000	
[4,1,1]	-6.000000	
```

</details>

## Bonus Tasks

**Implement the Buffer Pool Manager**

We already provide you a mock buffer pool manager and a table heap so that you do not need to interact with the disk and persist data to the disk. You can implement a real buffer pool manager based on [project 1](https://15445.courses.cs.cmu.edu/fall2023/project1/) of 15-445/645. Remember to revert both the buffer pool manager change and the table heap change before starting implementing the project, otherwise there will be memory leaks and deadlock issues.

**Implement Delete and Update**

You may implement the delete and update executor to update the data in the table heap and the vector indexes. When you delete or update an entry, BusTub does not actually remove the data from the table heap. Instead, it sets the deletion marker. Therefore, you can use the `UpdateTupleMeta` function when deleting a record, and convert update to a deletion followed by an insertion. Also remember to update the vector indexes if you implement these two executors. You might need to add new member functions to `VectorIndex` class to remove data from vector indexes.

**Insert Validation**

It is possible that a user might insert a vector of dimension 3 or 5 to a `VECTOR(4)` column. In insertion executor, you may do some validations to ensure the received tuples are of the correct schema.
