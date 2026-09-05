![Vector Database from Scratch — Build vector search, then use it from SQL](course/src/vectordb-social.png)

# Vector Database from Scratch

Vector Database from Scratch is a hands-on course for systems and database engineers. Build a small in-memory vector
database in Rust, compare approximate results with exact search, and use the resulting indexes from SQL through
DataFusion.

**[Read the course](https://skyzh.github.io/vector-db-from-scratch/)**

## What You Will Build

By the end of the course, your vector database will include:

* an Arrow-backed in-memory table and a safe DataFusion optimizer rule;
* IVFFlat, NSW, HNSW, and residual IVF-PQ indexes;
* SQL commands for creating indexes and comparing query plans; and
* a shared benchmark for recall, build time, and search latency on SIFT1M.

The course focuses on the boundary where vector-search algorithms become database features. It covers the algorithms,
evaluation contracts, query planning, and execution path instead of hiding them behind an HTTP API or ANN library.

## Course Structure

The [guided Rust course](https://skyzh.github.io/vector-db-from-scratch/rust-01-overview) has six days. Each day adds one
database or indexing capability, with starter code, focused tests, and a completed reference. The book contains the
setup, checkpoint commands, SQL walkthroughs, and benchmark instructions.

You need basic Rust, but you do not need prior knowledge of vector search or DataFusion.

The original [C++/BusTub edition](https://skyzh.github.io/vector-db-from-scratch/cpp-01-overview) remains online for
existing readers, but it is deprecated and unmaintained.

## Community

Join skyzh's Discord server to study with the Vector Database from Scratch community.

[![Join skyzh's Discord Server](https://skyzh.github.io/vector-db-from-scratch/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The code in this repository is licensed under the [Apache License 2.0](./LICENSE). The book, including its Markdown and
figures, is licensed under [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/).

The `bustub-vectordb-starter` and `bustub-vectordb-solution` git submodules retain their own upstream license terms.
