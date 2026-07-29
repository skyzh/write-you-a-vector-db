# Build Vector Search Inside a SQL Database

![Write You a Vector Database — Build vector search, then use it from SQL](vectordb-social.png)

Write You a Vector Database is a short hands-on course for systems and backend engineers. You will build a small in-memory
vector database in Rust, first with exact nearest-neighbor search and then with approximate indexes. The final system will
answer SQL top-k queries through DataFusion and make the tradeoff between recall, latency, and memory visible.

<div class="warning">

**Course status:** Days 1–2 are ready to implement: an in-memory Arrow table and DataFusion optimizer rule, followed by
IVFFlat. The repository includes learner starter code, focused tests, and separate completed references.

</div>

The original 2024 C++/BusTub course is preserved as a
[deprecated, unmaintained edition](./cpp-01-overview.md). It is no longer the recommended path for new learners.

## Why Build a Vector Database?

Embeddings turn text, images, and other data into fixed-dimensional vectors. A vector database stores those vectors and
retrieves the items closest to a query vector under a distance metric. Exact search compares the query with every stored
vector. Approximate nearest-neighbor (ANN) indexes avoid much of that work in exchange for returning an imperfect result
set.

This course builds vector search as a database feature rather than as an isolated ANN library. The goal is not only to
understand IVFFlat as an algorithm, but also to see how vectors, distance expressions, query planning, execution, and
indexes fit together behind one SQL top-k query.

## What You Will Build

The course has one cumulative Rust implementation:

1. An Arrow-backed vector table and a conservative DataFusion optimizer rule that selects a vector-index scan.
2. An IVFFlat index and recall harness that treat exact search as the correctness oracle.

The core is an ordinary Rust library. The learner-built DataFusion adapter turns a safe SQL top-k pattern into a
vector-index scan before the approximate index exists, while the collection and index remain independent of Arrow and
the query engine. DataFusion supplies exact distance, sort, and limit execution, so the Rust path does not duplicate an
exact k-nearest-neighbor executor chapter.

## Learning Goals

After completing the course, you should be able to:

- define the semantics and edge cases of Euclidean, cosine, and inner-product search;
- map stable vector rows into Arrow arrays and a DataFusion `TableProvider`;
- explain how IVFFlat trades build cost, memory, latency, and recall;
- design benchmarks that compare ANN results with exact ground truth;
- recognize when a SQL top-k query can safely use an approximate index; and
- separate a storage and search engine from its SQL interface.

## What This Course Will Not Cover

The required path will not implement embedding models, persistent index files, online updates or deletes after an index is
built, crash recovery, filtered ANN search, distributed execution, GPU kernels, or an HTTP service. It will also avoid
calling an existing ANN library for the algorithms students are meant to learn.

Those boundaries keep the course focused on vector search. They also make every required component small enough to test,
measure, and explain.

## Prerequisites

You should be comfortable with Rust ownership, traits, error handling, iterators, and Cargo. You should also know basic
database concepts such as records, indexes, SQL ordering, and query plans.

Prior knowledge of nearest-neighbor algorithms, Apache Arrow, or DataFusion is not required. Day 1 introduces the small
subset of DataFusion's extension interface used by the course.

## How to Use This Book

Start with the [Rust course](./rust-01-overview.md). It defines the architecture, system contracts, learner workspace,
progression, and scope. Both days pair the book with starter code, focused tests, and a separate reference solution.

Each implementation day begins with an observable capability, the relevant invariants, and a small prediction exercise.
It ends with focused verification and questions that require evidence from the implementation or benchmark rather than
recall from the prose.

Day 1 makes the table and optimizer rule runnable before Day 2's algorithm. IVFFlat is then compared with an exact oracle
and exercised through the same collection API and SQL query, with physical-plan and result evidence.

## Community

Join skyzh's Discord server to study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](discord-badge.svg)](https://skyzh.dev/join/discord)

## About the Author

Chi is a database systems engineer and the author of [Mini-LSM](https://skyzh.github.io/mini-lsm/) and
[LLM Serving in a Week](https://skyzh.github.io/tiny-llm/). He has worked on storage and database systems including TiKV,
AgateDB, TerarkDB, RisingWave, Neon, and RisingLight, and served as a teaching assistant for CMU's Database Systems course.

<div class="warning">

This course is not affiliated with Carnegie Mellon University or the CMU-DB Group. The deprecated C++ edition is not part
of CMU's 15-445/645 Database Systems course.

</div>

{{#include copyright.md}}
