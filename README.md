![Write You a Vector Database — Build vector search in Rust, then use it from SQL](course/src/vectordb-social.png)

# Write You a Vector Database: Build Vector Search in Rust

Write You a Vector Database is a short, hands-on systems course. Build exact k-nearest-neighbor search, IVFFlat, NSW, and
HNSW from scratch in Rust; measure every approximate index against an exact recall oracle; then use the same indexes from
SQL through Apache DataFusion.

**[Read the course](https://skyzh.github.io/write-you-a-vector-db/)**

The course is designed for systems and backend engineers who want to understand vector database internals instead of
calling an ANN library as a black box. The progression follows one causal path:

```text
exact search → recall and latency → IVFFlat → NSW → HNSW → SQL with DataFusion
```

The final chapter starts with an exhaustive SQL top-k plan and replaces it with a `VectorIndexScanExec` only when the query
preserves the index contract. `EXPLAIN` makes both the optimization and the exact fallback visible.

## Course Status

The cumulative reference implementation, focused tests, and executable chapters are available as a preview. Learner
starter/completed refs and recorded human review remain release requirements.

Run the implementation with:

```shell
cargo test --workspace
cargo run -p vector-datafusion --example sql
```

## What You Will Learn

- define L2, cosine, and inner-product search with deterministic top-k semantics;
- build exact search and use it as the ground truth for ANN evaluation;
- explain how IVFFlat, NSW, and HNSW spend candidate work differently;
- report recall together with latency instead of optimizing a misleading benchmark; and
- recognize when SQL can safely use an approximate index and when it must stay exhaustive.

## Legacy C++ Edition

The original BusTub-based C++ course remains in the book as an unmaintained legacy edition. Its starter and solution
submodules are preserved for existing readers, but new course development will focus on Rust.

## Community

You may join skyzh's Discord server and study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](course/src/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The BusTub vector-db starter code and solution are under the MIT license. Some files overlap with CMU's Database Systems
course and must not be made public. The author reserves the full copyright of the book's Markdown files and figures.
