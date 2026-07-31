![Write You a Vector Database — Build vector search, then use it from SQL](https://skyzh.github.io/write-you-a-vector-db/vectordb-social.png)

# Write You a Vector Database

Write You a Vector Database is a short, Rust-first systems course. Build a small in-memory vector database in Rust,
compare approximate results with exact search, and connect the resulting indexes to SQL through DataFusion.

**[Read the course](https://skyzh.github.io/write-you-a-vector-db/)**

The course focuses on the boundary where algorithms become database features:

```text
in-memory table → DataFusion optimizer rule → IVFFlat → NSW → HNSW
```

Instead of hiding vector search behind an HTTP API or an ANN library, the course exposes the algorithms, evaluation
contracts, query planning, and execution boundary that make SQL vector search work.

## Course Status

The [Rust course](https://skyzh.github.io/write-you-a-vector-db/rust-01-overview) has four chapters: an Arrow-backed
in-memory table and safe DataFusion optimizer rule, followed by IVFFlat, NSW, and HNSW. Each chapter includes starter code,
focused tests, SQLLogicTests, and separate completed reference crates.

Run the completed reference with:

```shell
cd rust
cargo test -p vector-core -p vector-datafusion
cargo run --release -p vector-core --example recall
```

The original [C++/BusTub edition](https://skyzh.github.io/write-you-a-vector-db/cpp-01-overview) is deprecated and
unmaintained. It remains online for existing readers but is no longer recommended for new learners.

## Community

Join skyzh's Discord server to study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](https://skyzh.github.io/write-you-a-vector-db/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The BusTub vector-db starter code and solution are under the MIT license. Some files overlap with CMU's Database Systems
course and must not be made public. The author reserves the full copyright of the course materials, including Markdown
files and figures.
