![Write You a Vector Database — Build vector search, then use it from SQL](https://skyzh.github.io/write-you-a-vector-db/vectordb-social.png)

# Write You a Vector Database

Write You a Vector Database is a short, Rust-first systems course. Build a small in-memory vector database in Rust,
compare approximate results with exact search, and connect the resulting indexes to SQL through DataFusion.

**[Read the course](https://skyzh.github.io/write-you-a-vector-db/)**

The course focuses on the boundary where algorithms become database features:

```text
in-memory table → DataFusion optimizer rule → IVFFlat → NSW → HNSW → IVF-PQ → benchmark
```

Instead of hiding vector search behind an HTTP API or an ANN library, the course exposes the algorithms, evaluation
contracts, query planning, and execution boundary that make SQL vector search work.

## Course Status

The [Rust course](https://skyzh.github.io/write-you-a-vector-db/rust-01-overview) has six required chapters: an
Arrow-backed in-memory table and safe DataFusion optimizer rule, IVFFlat, NSW, HNSW, residual IVF-PQ, and a final shared
recall and latency benchmark. The implementation chapters include starter code, focused tests, and separate completed
reference crates. Chapters 1–5 include self-contained SQLLogicTests; Chapter 5 also includes a focused planner/EXPLAIN
test.

The final benchmark compares Flat, IVFFlat, NSW, HNSW, and IVF-PQ on the external SIFT1M corpus. All five indexes share
the same Euclidean queries and `k = 100`. The report couples build and search time with first-neighbor rank recall at
1, 10, and 100, p50 and p99 search latency, and explicit IVF-PQ search-representation accounting.

## Rust Workspace

The repository-root Cargo workspace separates the learner starter from the completed reference:

```text
vector-db-starter/
  core/          package: vector-core-starter
  datafusion/    package: vector-datafusion-starter
vector-db/
  core/          package: vector-core
  datafusion/    package: vector-datafusion
vector-db-benchmark-support/
                shared benchmark fixtures and reporting support
```

Before implementing Chapter 1, launch the supplied product shell from the repository root:

```shell
cargo run -p vector-datafusion --example sql
```

The supplied DataFusion CLI starts with an empty course session and accepts semicolon-terminated SQL. The product tour
creates and populates an in-memory table, compares a query with `EXPLAIN`, attaches an index named in SQL, and runs the
same query again. You do not need to inspect or modify the completed reference example.

Check the untouched starter without executing TODOs:

```shell
cargo check -p vector-core-starter
cargo check -p vector-datafusion-starter
```

Validate the completed reference with:

```shell
cargo test -p vector-core -p vector-datafusion
cargo run --release -p vector-core --example recall -- /absolute/path/to/sift1M
```

The workspace uses the stable Rust channel from `rust-toolchain.toml` and pins course dependencies in `Cargo.lock`.

Each chapter ends with two day-based learner commands. The first runs only the
new tests for that day; the second reruns every learner day through it:

```shell
cargo x test-day 1
cargo x test-through 1
```

Replace `1` with the current day number, from 1 through 6. The Day 6 SIFT1M
tests remain ignored unless you explicitly provide the external corpus and use
their chapter commands.

The original [C++/BusTub edition](https://skyzh.github.io/write-you-a-vector-db/cpp-01-overview) is deprecated and
unmaintained. It remains online for existing readers but is no longer recommended for new learners.

## Community

Join skyzh's Discord server to study with the write-you-a-vector-db community.

[![Join skyzh's Discord Server](https://skyzh.github.io/write-you-a-vector-db/discord-badge.svg)](https://skyzh.dev/join/discord)

## License

The code in this repository is licensed under the [Apache License 2.0](./LICENSE). The book, including its Markdown and
figures, is licensed under [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/).

The `bustub-vectordb-starter` and `bustub-vectordb-solution` git submodules retain their own upstream license terms.
