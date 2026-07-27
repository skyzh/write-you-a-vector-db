![banner](tutorial/src/vectordb-banner-horizontal.png)

# Write You a Vector Database

Write You a Vector Database is being redesigned as a short, Rust-first hands-on course. The new course builds a small
vector search engine as a Rust library and integrates it with SQL through a thin DataFusion adapter.

The executable reference preview now includes deterministic exact search, a recall harness, IVFFlat, NSW, HNSW, and a
DataFusion 54.1.0 table provider that pushes compatible vector top-k queries into the selected index. Learner starter and
completed checkpoint refs have not been published, so the Rust course is still marked as a preview rather than ready.

Run the implementation with:

```shell
cargo test --workspace
cargo run -p vector-datafusion --example sql
```

Read the course at [https://skyzh.github.io/write-you-a-vector-db](https://skyzh.github.io/write-you-a-vector-db).

## Legacy C++ Edition

The original BusTub-based C++ course remains in the book as an unmaintained legacy edition. Its starter and solution
submodules are preserved for existing readers, but new course development will focus on Rust.

## Community

You may join skyzh's Discord server and study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](tutorial/src/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The BusTub vector-db starter code and solution are under the MIT license. Some files overlap with CMU's Database Systems
course and must not be made public. The author reserves the full copyright of the book's Markdown files and figures.
