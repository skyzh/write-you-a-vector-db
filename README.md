![Write You a Vector Database — Build vector search, then use it from SQL](https://skyzh.github.io/write-you-a-vector-db/vectordb-social.png)

# Write You a Vector Database

Write You a Vector Database is a short, Rust-first systems course. Build exact and approximate vector search from scratch,
measure recall honestly, and connect the resulting indexes to SQL through DataFusion.

**[Read the course](https://skyzh.github.io/write-you-a-vector-db/)**

The course focuses on the boundary where algorithms become database features:

```text
exact search → recall-first evaluation → IVFFlat → NSW → HNSW → SQL through DataFusion
```

Instead of hiding vector search behind an HTTP API or an ANN library, the course exposes the algorithms, evaluation
contracts, query planning, and execution boundary that make SQL vector search work.

## Course Status

The [Rust course design proposal](https://skyzh.github.io/write-you-a-vector-db/rust-01-overview) defines the recommended
direction: a standalone vector core with a thin DataFusion adapter. Runnable learner checkpoints have not been published
yet, so the Rust edition remains a design proposal rather than an available assignment sequence.

The original [C++/BusTub edition](https://skyzh.github.io/write-you-a-vector-db/cpp-01-overview) is deprecated and
unmaintained. It remains online for existing readers but is no longer recommended for new learners.

## Community

Join skyzh's Discord server to study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](https://skyzh.github.io/write-you-a-vector-db/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The BusTub vector-db starter code and solution are under the MIT license. Some files overlap with CMU's Database Systems
course and must not be made public. The author reserves the full copyright of the course materials, including Markdown
files and figures.
