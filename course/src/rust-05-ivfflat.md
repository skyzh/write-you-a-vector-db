# Restrict Search with IVFFlat

> **Chapter ID:** `VDB-IVF`
>
> **Prerequisite:** `VDB-SQL`
>
> **Status:** executable reference preview; human review unrecorded

Before this chapter, exact search visits every point. After it, seeded k-means
partitions the dataset into inverted lists and `probes` controls how many lists
a query scans.

## Mental Model

Build time assigns every point to its nearest centroid. Query time ranks the
centroids, opens the nearest lists, and applies the exact metric only to those
candidates. Raising `probes` spends more work to improve recall.

The IVFFlat diagrams from the C++ edition express the same build stages as the
Rust implementation: choose centroids, then assign every row to one list.

![Vectors before IVFFlat clustering](./vector-db/04-ivfflat-step1.svg)

![K-means chooses centroids and their Voronoi regions](./vector-db/04-ivfflat-step2.svg)

![Every vector is assigned to its nearest centroid](./vector-db/04-ivfflat-step3.svg)

At query time, scanning only the nearest list can miss close rows across a
partition boundary. Increasing `probes` opens neighboring lists as well.

![Probing one centroid can miss a nearby point in another list](./vector-db/04-ivfflat-lookup.svg)

![Probing two centroids expands the candidate set](./vector-db/04-ivfflat-lookup-2.svg)

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
cargo test -p vector-datafusion --test sqllogictest
```

Stop when I1–I4 hold. Do not add persistent postings, online centroid updates,
product quantization, or a fixed latency target.

Explain back why probing every list is an important break test, what happens to
recall as probes approaches partitions, and why an empty cluster cannot simply
retain an all-zero cosine centroid.
