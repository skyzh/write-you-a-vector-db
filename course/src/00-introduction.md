# Build Vector Search in Rust—and Use It from SQL

![Write You a Vector Database — Build vector search in Rust, then use it from SQL](vectordb-social.png)

Write You a Vector Database is a short, hands-on systems course. You will build exact k-nearest-neighbor search, IVFFlat,
NSW, and HNSW from scratch in Rust; measure every approximate index against an exact recall oracle; then use the same
indexes from SQL through Apache DataFusion.

The course is for systems and backend engineers who want to understand vector database internals instead of calling an ANN
library as a black box. One cumulative implementation connects the algorithms to the database boundary:

```text
exact search → recall and latency → IVFFlat → NSW → HNSW → SQL with DataFusion
```

**[Start with the Rust course overview](./rust-01-overview.md)** to see the complete learning path.

<div class="warning">

**Course status:** The cumulative reference implementation, focused tests, and executable chapters are available as a
preview. Learner starter/completed refs and recorded human review remain release requirements.

</div>

The original 2024 C++ course is preserved as an [unmaintained legacy edition](./cpp-01-overview.md). It is no longer the
path we recommend to new learners.

## Why Build Vector Search from Scratch?

Embeddings turn text, images, and other data into fixed-dimensional vectors. A vector database stores those vectors and
retrieves the items closest to a query vector under a distance metric. Exact search compares the query with every stored
vector. Approximate nearest-neighbor (ANN) indexes avoid much of that work in exchange for returning an imperfect result
set.

That exchange makes vector search a useful systems course. A result can be valid but have poor recall; an index can make
queries faster while making writes, memory use, or recovery harder; and a benchmark can look impressive while measuring
the wrong workload. The implementation is small enough to understand, but the engineering choices are real.

## What You Will Build

The course has one cumulative Rust implementation:

1. An exact-search collection with stable point IDs and a bounded top-k operator.
2. A benchmark and recall harness that treats exact search as the correctness oracle.
3. IVFFlat, NSW, and HNSW indexes behind the same search interface.
4. A thin DataFusion adapter that turns a safe SQL top-k pattern into a vector-index scan.

The core is an ordinary Rust library. DataFusion supplies SQL parsing, planning, Arrow arrays, and execution, but the
collection and index remain independent of it. This separation makes the integration small enough to understand and shows
which responsibilities belong to the SQL engine and which belong to the vector index.

## Learning Goals

After completing the course, you should be able to:

- define the semantics and edge cases of L2, cosine, and inner-product search;
- implement exact top-k search without sorting the entire collection;
- explain how IVFFlat and HNSW trade build cost, memory, latency, and recall;
- design benchmarks that compare ANN results with an exact ground truth;
- recognize when a SQL top-k query can safely use an approximate index; and
- separate a storage and search engine from its SQL interface.

## What This Course Will Not Cover

The required path does not implement embedding models, persistent index files, online updates or deletes after an index is
built, crash recovery, filtered ANN search, distributed execution, GPU kernels, or an HTTP service. It also avoids
calling an existing ANN library for the algorithms students are meant to learn.

Those boundaries keep the course focused on vector search. They also make every required component small enough to test,
measure, and explain.

## Prerequisites

You should be comfortable with Rust ownership, traits, error handling, iterators, and Cargo. You should also know basic
database concepts such as records, indexes, SQL ordering, and query plans.

Prior knowledge of nearest-neighbor algorithms, Apache Arrow, or DataFusion is not required. The course will introduce the
small subset of DataFusion's extension interface used by the final chapter.

## How to Use This Book

The implementation is the laboratory. Each chapter begins with a capability, a small set of invariants, and a prediction
exercise. It ends with focused tests, a chapter checkpoint, and questions that require evidence from the
implementation or benchmark rather than recall from the prose.

Every chapter starts from the runnable checkpoint produced by the previous chapter. New algorithms are first compared with
an exact oracle and then integrated behind the existing collection API. The same SQL query shown in the opening chapter
runs through the ANN index in the final chapter. A later chapter never becomes an undeclared
prerequisite for an earlier one.

The [Rust course overview](./rust-01-overview.md) shows the complete learning path and the current preview boundary.

## Community

You may join skyzh's Discord server and study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](discord-badge.svg)](https://skyzh.dev/join/discord)

## About the Author

Chi is a database systems engineer and the author of [Mini-LSM](https://skyzh.github.io/mini-lsm/) and
[LLM Serving in a Week](https://skyzh.github.io/tiny-llm/). He has worked on storage and database systems including TiKV,
AgateDB, TerarkDB, RisingWave, Neon, and RisingLight.

{{#include copyright.md}}
