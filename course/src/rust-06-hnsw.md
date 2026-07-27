# Add Hierarchy with HNSW

> **Chapter ID:** `VDB-HNSW`
>
> **Prerequisite:** `VDB-NSW`
>
> **Status:** executable reference preview; human review unrecorded

Before this chapter, every search navigates one dense graph. After it, seeded
random levels create sparse upper layers for coarse routing and retain NSW beam
search at layer zero.

## Contract

Relevant code is `rust/vector-core/src/hnsw.rs`; shared traversal remains in
`graph.rs`.

1. **I1 — Membership:** a row present at level `L` is present at every level
   from zero through `L`.
2. **I2 — Seeded levels:** identical data, configuration, and seed produce the
   same level sequence and top level.
3. **I3 — Descent:** query performs greedy single-entry descent on upper layers,
   then best-first search with `ef_search` at layer zero.
4. **I4 — Bounded degree:** every layer applies the configured connection bound
   and deterministic pruning.

## Checkpoints

1. Sample a capped geometric level for each inserted row.
2. Maintain the entry point with the highest observed level.
3. Greedily descend above the new row's level during insertion.
4. Search and connect each shared layer from top to bottom.
5. Reuse the NSW layer-zero search and evaluate recall.

## Verification

Run:

```sh
cargo test -p vector-core hnsw_is_seeded_and_high_ef_recovers_neighbors
cargo run --release -p vector-core --example recall
```

The benchmark is informational. Stop when I1–I4 and the focused test hold; do
not add persistence, deletion, concurrent mutation, or production HNSW neighbor
diversification.

Explain back why upper layers may use greedy search while layer zero uses a
beam, when the global entry point changes, and how the seed affects evaluation.
