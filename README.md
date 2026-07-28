![Write You a Vector Database — Build vector search, then use it from SQL](course/src/vectordb-social.png)

# Write You a Vector Database

Write You a Vector Database is a hands-on course for systems and backend engineers. Build pgvector-style vector storage
and similarity search inside a relational database, then implement IVFFlat, NSW, and HNSW and teach the query optimizer to
use those indexes for SQL top-k queries.

**[Read the course](https://skyzh.github.io/write-you-a-vector-db/)**

The course focuses on the boundary where algorithms become database features:

```text
vector storage → exact k-nearest neighbors → index selection → IVFFlat → NSW → HNSW
```

Instead of hiding vector search behind an HTTP API or an ANN library, the exercises expose the type system, execution
engine, optimizer, and index internals that make SQL vector search work.

## Course Editions

The current implementation path uses C++17 and a modified version of CMU-DB's
[BusTub](https://github.com/cmu-db/bustub) educational database. A separate
[Rust course design proposal](course/src/rust-01-overview.md) defines a short path that keeps the vector indexes in a
standalone crate and connects them to SQL through DataFusion. The Rust edition is not yet part of the published course.

By the end of the published path, you will have implemented:

- vector values, distance expressions, and compact storage;
- deterministic exact top-k execution;
- optimizer matching for compatible vector-index queries;
- an IVFFlat index built with k-means clustering; and
- graph search and construction through NSW and HNSW.

## Community

Join skyzh's Discord server to study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](course/src/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The BusTub vector-db starter code and solution are under the MIT license. Some files overlap with CMU's Database Systems
course and must not be made public. The author reserves the full copyright of the course materials, including Markdown
files and figures.
