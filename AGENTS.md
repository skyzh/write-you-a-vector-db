# Course Repository Instructions

## Status and working mode

The Rust implementation is an executable preview. The cumulative reference
implementation and tests are present, but learner starter/completed refs have
not been published. Work in one of these modes:

- `learner`: follow one chapter contract and do not search Git history or
  external solution branches for the completed implementation;
- `maintainer`: update the book, implementation, or tests while keeping their
  contracts aligned;
- `evaluator`: review behavior and evidence without editing files.

If no mode is named, use `maintainer` for repository changes and `evaluator`
for reviews.

## Source of truth

1. A chapter's required behavior, invariants, and error semantics are normative.
2. Tests are executable evidence but may not cover the entire contract.
3. Public APIs define the available integration surfaces.
4. Narrative prose, diagrams, examples, and hints are explanatory.
5. Reference implementations and agent run logs are non-normative.

Report a conflict between these levels instead of weakening the higher-level
requirement.

## Repository boundaries

- `rust/vector-core` owns datasets, metrics, exact search, IVFFlat, NSW, HNSW,
  and recall measurement. It must not depend on Arrow or DataFusion.
- `rust/vector-datafusion` owns Arrow conversion, the table provider, SQL
  pattern recognition, and execution-plan metadata.
- `tutorial/src` is the mdBook source. `tutorial/book` is generated and must not
  be edited.
- The `bustub-vectordb-*` submodules are the unmaintained legacy C++ course.
- Do not weaken fallback tests: unsupported SQL shapes must retain DataFusion's
  exhaustive plan.

## Canonical verification

Run focused checks while editing, then before completion run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p vector-datafusion --example sql
mdbook build tutorial
```

The release-mode recall example is informational rather than a correctness
gate:

```sh
cargo run --release -p vector-core --example recall
```

Report commands actually run, unrun checks, known limitations, and provenance.
