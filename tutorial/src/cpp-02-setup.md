# Vector Storage + Expressions

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

## Insertion and Sequential Scan

## Bonus Tasks

We provide you mock buffer pool manager and table heap. You can implement a real buffer pool manager based on project 1 of 15-445/645.
