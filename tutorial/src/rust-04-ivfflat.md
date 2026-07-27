# Restrict Search with IVFFlat

> **Chapter ID:** `VDB-IVF`
>
> **Prerequisite:** `VDB-EVAL`
>
> **Status:** executable reference preview; human review unrecorded

Before this chapter, exact search visits every point. After it, seeded k-means
partitions the dataset into inverted lists and `probes` controls how many lists
a query scans.

## Mental Model

Build time assigns every point to its nearest centroid. Query time ranks the
centroids, opens the nearest lists, and applies the exact metric only to those
candidates. Raising `probes` spends more work to improve recall.

## Contract

Relevant code is `rust/vector-core/src/ivf.rs`.

1. **I1 — Complete assignment:** every dataset row appears in exactly one
   inverted list.
2. **I2 — Valid budget:** `1 <= probes <= partitions <= rows` and iterations
   are positive.
3. **I3 — Seeded build:** the same data, metric, configuration, and seed produce
   the same centroids and list sizes.
4. **I4 — Exact limit:** probing every partition returns the same top-k as
   `FlatIndex`, including tie order.

Empty clusters are re-seeded from a far-away point instead of silently leaving
an invalid centroid. Cosine centroids are normalized after averaging.

## Checkpoints

1. Choose seeded initial centroids.
2. Alternate assignment and centroid update, handling empty clusters.
3. Materialize one posting list per final centroid.
4. Rank centroids at query time and scan the selected candidate lists.
5. Plot or record the recall/latency change as `probes` increases.

## Verification

Run:

```sh
cargo test -p vector-core ivf_build_is_seeded
cargo test -p vector-core ivf_scanning_every_partition_matches_exact_search
```

Stop when I1–I4 hold. Do not add persistent postings, online centroid updates,
product quantization, or a fixed latency target.

Explain back why probing every list is an important break test, what happens to
recall as probes approaches partitions, and why an empty cluster cannot simply
retain an all-zero cosine centroid.
