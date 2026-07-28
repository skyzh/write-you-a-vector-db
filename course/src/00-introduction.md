# Build Vector Search Inside a SQL Database

![Write You a Vector Database — Build vector search, then use it from SQL](vectordb-social.png)

Write You a Vector Database is a hands-on course about the internals behind SQL vector search. You will add vector values
and similarity expressions to a relational database, implement exact k-nearest-neighbor execution, build approximate
indexes from scratch, and connect those indexes to the query optimizer.

The published course follows one cumulative path:

```text
vector storage → exact k-nearest neighbors → index selection → IVFFlat → NSW → HNSW
```

**[Start with the course overview](./cpp-01-overview.md)** to see the chapter dependencies and implementation boundary.

## Why Build Vector Search from Scratch?

Embeddings represent text, images, and other data as fixed-dimensional vectors. A vector database retrieves the items
closest to a query vector under a distance metric. Exact search compares the query with every stored vector. Approximate
nearest-neighbor indexes avoid much of that work in exchange for returning an imperfect result set.

PostgreSQL with [pgvector](https://github.com/pgvector/pgvector) exposes these capabilities through ordinary database
objects and SQL:

```sql
CREATE TABLE items (id bigserial PRIMARY KEY, embedding vector(3));
SELECT * FROM items ORDER BY embedding <-> '[3,1,2]' LIMIT 5;
CREATE INDEX ON items USING hnsw (embedding vector_l2_ops);
```

This course builds the same shape of functionality inside an educational relational database. The goal is not only to
understand IVFFlat and HNSW as algorithms, but also to see how vector types, expressions, storage, execution, optimization,
and indexes fit together behind one SQL query.

## What You Will Build

The published implementation path uses C++17 and a modified version of CMU-DB's
[BusTub](https://github.com/cmu-db/bustub) educational database. Across the course, you will implement:

1. vector values, distance expressions, and compact storage;
2. deterministic exact top-k execution;
3. optimizer rules that match compatible SQL queries with vector indexes;
4. an IVFFlat index built with k-means clustering; and
5. graph search and construction through NSW and HNSW.

The exercises implement the index algorithms directly instead of delegating them to Faiss or another ANN library. They
also keep SQL as the integration surface instead of spending the course on HTTP routing and serialization.

## Course Status

The C++/BusTub edition is the current published path. The separate
[Rust course design proposal](./rust-01-overview.md) specifies a standalone vector core and a thin DataFusion SQL adapter.
The Rust edition will be published only when its implementation, tests, and learner checkpoints are available.

## Prerequisites

You should know basic relational database concepts and be comfortable with systems programming. Prior experience with
vector search or database internals is not required.

The published path uses modern C++ and BusTub's C++17 codebase. If you need a refresher on the language features used by
BusTub, complete the [C++ primer](https://15445.courses.cs.cmu.edu/fall2023/project0/) from CMU's Database Systems course.

## Solution and Publication Rules

A solution is available on the `vectordb-solution` branch of
[skyzh/bustub-vectordb](https://github.com/skyzh/bustub-vectordb), except for material that overlaps with CMU's Database
Systems course.

<div class="warning">

Some exercises overlap with Carnegie Mellon University's Database Systems course. Follow each chapter's instructions
about which parts of your implementation may be published.

</div>

## Community

Join skyzh's Discord server to study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](discord-badge.svg)](https://skyzh.dev/join/discord)

## About the Author

Chi is a database systems engineer and the author of [Mini-LSM](https://skyzh.github.io/mini-lsm/) and
[LLM Serving in a Week](https://skyzh.github.io/tiny-llm/). He has worked on storage and database systems including TiKV,
AgateDB, TerarkDB, RisingWave, Neon, and RisingLight, and served as a teaching assistant for CMU's Database Systems course.

<div class="warning">

This course is not affiliated with Carnegie Mellon University or the CMU-DB Group. Its C++ edition is not part of CMU's
15-445/645 Database Systems course.

</div>

{{#include copyright.md}}
