# C++ Course over BusTub (Deprecated)

{{#include cpp-deprecation.md}}

In this edition, you will add vector search to a modified version of BusTub, CMU's educational database system. The last
fully specified checkpoint is a one-layer NSW index. The HNSW and large-dataset pages are optional work-in-progress notes,
not complete assignments.

## Course Order

Follow the chapters in order. Each runnable checkpoint depends on the previous one:

1. implement vector distances, insertion, and sequential scan;
2. implement exact k-nearest-neighbor queries with sort, limit, and Top-N;
3. match a safe SQL top-k query to a compatible vector index;
4. implement IVFFlat; and
5. implement a one-layer NSW graph.

The diagram shows the same dependencies. It is useful as a map, but it does not make the chapters independent.

![Learning Path](./vector-db/01-learn-path.svg)

## Environment Setup

Use the course's pinned 2024 starter. Commands and interfaces in these chapters were checked against commit
`74de667e5d168f14fff9c9ea23af246a85f9785f`.

```shell
git clone https://github.com/skyzh/bustub-vectordb
cd bustub-vectordb
git checkout 74de667e5d168f14fff9c9ea23af246a85f9785f
```

The intended environments are Ubuntu 22.04 and macOS. Follow the starter repository's **Build** section to install its
platform packages. The project uses CMake, C++17, and LLVM/Clang 14. New Apple Clang releases are not a compatible
substitute: the starter treats deprecation warnings from its 2024 dependencies as errors.

From the `bustub-vectordb` directory, create a build directory:

```shell
mkdir build
cd build
```

On Ubuntu, configure with Clang 14:

```shell
cmake -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -DCMAKE_C_COMPILER=clang-14 \
  -DCMAKE_CXX_COMPILER=clang++-14 \
  ..
```

On macOS with Homebrew's `llvm@14`, configure with:

```shell
cmake -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -DCMAKE_C_COMPILER="$(brew --prefix llvm@14)/bin/clang" \
  -DCMAKE_CXX_COMPILER="$(brew --prefix llvm@14)/bin/clang++" \
  ..
```

Then build the two course binaries:

```shell
make -j8 shell sqllogictest
```

The policy option lets the starter's older vendored CMake projects configure under CMake 4. All later build and test
commands assume that your working directory is `bustub-vectordb/build`.

Run the SQL shell:

```console
$ ./bin/bustub-shell
bustub> SELECT ARRAY [1.0, 2.0, 3.0];
+-------------+
| __unnamed#0 |
+-------------+
| [1,2,3]     |
+-------------+
```

In this starter, an `ARRAY` expression becomes a vector only when every element is a decimal literal such as `1.0`.
Integer literals such as `1` are outside the required path.

## What the Starter Adds

The starter narrows BusTub to the parts used by this course:

- **In-memory table storage.** A modified table heap and buffer pool keep the course data in memory.
- **Vector expressions.** The parser, type system, and expression tree already recognize three vector-distance operations.
- **Vector-index interfaces.** `VectorIndex`, `IVFFlatIndex`, and `HNSWIndex` connect index construction, insertion, and lookup.
- **Vector-index execution.** A plan node and executor can turn ordered vector-index RIDs back into table tuples.

Some executor work overlaps with CMU's Database Systems assignments. Keep the files marked **KEEP PRIVATE** in a private
repository and follow the academic-integrity notice in the starter repository.

## How to Check Each Chapter

The `vector.*.slt` files use `statement ok`, so they mainly prove that a statement ran without an error. Their verbose
output is an inspection aid, not a complete correctness oracle. Where a stricter BusTub SQLLogicTest exists, the chapter
names it. For every checkpoint, also explain:

- how a tuple or query moves through the code you changed;
- the invariant that keeps its result correct;
- one input that could break a careless implementation; and
- which test would expose that failure.

{{#include copyright.md}}
