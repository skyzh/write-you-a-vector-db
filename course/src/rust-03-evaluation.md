# Measure Recall Before Optimizing

> **Chapter ID:** `VDB-EVAL`
>
> **Prerequisite:** `VDB-EXACT`
>
> **Status:** executable reference preview; benchmark is informational

Before this chapter, a search result can look plausible without evidence.
After it, every approximate result is compared with exact top-k on the same
query set and reported with latency.

## Goal and Boundaries

Build a seeded, in-process evaluation fixture. Record dataset size, dimension,
query count, `k`, build time, recall, and p50/p99 query latency. The fixture in
`rust/vector-core/examples/recall.rs` is intentionally synthetic and small; it
tests the measurement loop, not product performance.

## Contract

1. **I1 — Same workload:** exact and approximate indexes receive identical
   vectors, queries, metric, and `k`.
2. **I2 — Recall denominator:** recall@k divides matching row IDs by the number
   of available exact neighbors up to `k`.
3. **I3 — Reproducibility:** data and index construction use named seeds or a
   deterministic generator.
4. **I4 — Honest output:** timings are observations tied to this process and
   machine, not correctness gates or general performance claims.

A latency number without recall can reward an index that returns arbitrary
rows. A recall number generated from a different metric or query set is equally
invalid.

## Checkpoints

1. Generate the dataset and query set deterministically.
2. Compute exact ground truth once, outside ANN timing.
3. Time build and each query separately.
4. Sort per-query durations for percentiles and pair them with mean recall.

## Verification

Run:

```sh
cargo test -p vector-core ivf_scanning_every_partition_matches_exact_search
cargo run --release -p vector-core --example recall
```

The second command's exact timings will vary. It must name the workload and
report recall in `[0, 1]`. Stop after the harness can compare index settings;
do not tune parameters to one machine or introduce a required external dataset.

Explain back how a benchmark can become faster while getting worse, why build
time is separate from query latency, and which metadata another engineer needs
to reproduce the observation.
