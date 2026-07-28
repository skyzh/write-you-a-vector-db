# Exact Search and Ground Truth

> **Chapter ID:** `VDB-EXACT`
>
> **Status:** executable reference preview; human review unrecorded

Before this chapter, vectors are ordinary lists of numbers. After it, vector
dimensions and metrics have explicit semantics and exact top-k is the oracle
for every later index.

## Goal and Boundaries

Implement a fixed-dimension `Dataset`, Euclidean/cosine/dot metrics, and a
bounded top-k search with deterministic tie-breaking. Stored values are finite
`f32`; distance accumulation uses `f64`. Cosine datasets and queries reject
zero-norm vectors.

Persistence, SQL planning, approximate search, and online mutation are
non-goals. The next SQL chapter will put this exact index behind DataFusion
before any approximation is introduced. Relevant code is under
`rust/vector-core/src`:

- `dataset.rs` validates representation and query boundaries;
- `metric.rs` defines the three ordering functions;
- `search.rs` owns deterministic neighbor ordering and the bounded heap; and
- `flat.rs` establishes exhaustive ground truth.

## Contract

1. **I1 — Dimension:** every stored and query vector has the dataset dimension.
2. **I2 — Total ordering:** lower `Neighbor::distance` is better; ties use the
   row offset so repeated runs return the same order.
3. **I3 — Metric mapping:** Euclidean and cosine sort ascending. Dot search
   stores negative inner product so the same lower-is-better heap works.
4. **I4 — Ground truth:** exact search visits every row and returns at most
   `min(k, rows)` unique neighbors.

Trace this boundary case before coding: for points `[1, 0]` and `[-1, 0]` and
query `[0, 0]`, both Euclidean distances are one, so row zero must precede row
one. A heap whose tie order depends on insertion or hash iteration violates I2.

## Checkpoints

1. Validate an ordinary dataset and reject empty, ragged, or non-finite input.
2. Work each metric by hand, including the sign change for dot product.
3. Add the top-k heap and verify deterministic ties.
4. Make `FlatIndex` the shared oracle used by later evaluations.

## Verification

Run:

```sh
cargo test -p vector-core flat_search_is_deterministic_and_validates_queries
cargo test -p vector-core cosine_rejects_zero_norm_vectors
```

The tests do not prove performance. Stop when I1–I4 hold; do not add SIMD,
quantization, persistence, or parallel scan in this chapter.

Explain back why negative dot product belongs in the metric boundary, which
incorrect heap comparison would lose deterministic ties, and why exact search
must remain available after ANN indexes exist.
