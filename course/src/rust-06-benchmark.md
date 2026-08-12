# Benchmark Five Indexes

{{#include rust-in-progress.md}}

> **Chapter 6**
>
> Complete [Compress IVFFlat with Product Quantization](./rust-07-ivfpq.md) first. Finish with one release-mode benchmark
> that compares Flat, IVFFlat, NSW, HNSW, and IVF-PQ on the same data, queries, Euclidean metric, and top-k contract.

An approximate index is useful only when its search time is attached to result quality. A latency number alone cannot
tell you whether the index found the right neighbors, and recall from a different workload cannot explain the latency you
measured.

This chapter gives every index the same immutable dataset and query vectors. Flat search produces exact top-k ground
truth before timing begins. Each index result is compared with those row offsets using recall@k, while construction and
search are measured separately.

## Freeze One Shared Workload

The supplied example fixes the complete comparison contract:

```text
rows        2,000
dimensions  16
queries     100
metric      euclidean
k           10
```

Dataset row `r` uses the deterministic `sample(r, dimension)` generator. Query `q` uses the separate generator domain
`2_000 + 17 * q + 3`, so query vectors are repeatable, unique, and disjoint from dataset rows.

Keep these five configurations unchanged:

| Index | Configuration |
| --- | --- |
| Flat | Exact search over every row |
| IVFFlat | 32 partitions, 6 probes, 12 iterations, seed 7 |
| NSW | 12 connections, construction width 64, search width 40 |
| HNSW | 12 connections, construction width 64, search width 40, maximum level 12, seed 7 |
| IVF-PQ | 32 partitions, 6 probes, 12 iterations, 4 subquantizers, 16 codewords, rerank 100, seed 7 |

This fixture is intentionally small enough to run locally. It does not establish production latency, a universal index
ranking, or behavior on real embedding distributions.

## Keep Construction and Search Boundaries Honest

Build Flat first and compute all exact ground-truth results outside every timer. For each index, create its dataset clone,
metric, and configuration before starting the construction timer. The measured build region contains only the index
constructor.

Search uses two passes:

1. one untimed warm-up pass; and
2. one timed pass with only `index.search(query, k)` inside each timer.

Both passes rotate index order for every query. At query `q`, visit index position `(q + offset) % 5`. Across 100 queries,
each index appears exactly 20 times in every position. This balanced cyclic order prevents one index from always running
first or last.

Dataset generation, index construction, exact-ground-truth creation, recall calculation, sorting, formatting, and
printing stay outside search timers.

**Prediction:** Flat recall must be exactly `1.000`. If it is not, which ground-truth or reporting boundary has failed?

## Compute Recall and Percentiles

For every query, compute recall@10 from the expected and actual row offsets. The reported recall is the arithmetic mean
of all 100 per-query values, not recall from one representative query.

Sort each index's 100 search durations before selecting nearest-rank percentiles. For percentage `p` and `n` samples, the
zero-based index is:

```text
(p * n).div_ceil(100).saturating_sub(1)
```

Clamp it to the final sample. With 100 durations, p50 selects index 49 and p99 selects index 98.

Timing varies across machines and runs. Treat durations as observations, not pass/fail thresholds. The fixed workload and
seeded builds make correctness fields repeatable, but do not require one approximate index to be faster or more accurate
than another.

## Build the Final Benchmark

You will modify:

```text
rust/vector-starter/core/examples/recall.rs
```

The example already fixes the workload, index inventory, configurations, reporting shape, and accounting boundary.
Complete its six Chapter 6 TODOs without changing index implementations or public APIs:

- construct NSW, HNSW, and IVF-PQ from the supplied configurations;
- run the balanced cyclic warm-up pass;
- run the balanced cyclic timed pass; and
- select a nearest-rank percentile.

All commands exercise the cumulative starter. Complete Chapters 1–5 first; otherwise an earlier `todo!()` may stop the
example before the Chapter 6 path runs.

### Checkpoint 1: Construct the Three Remaining Indexes

Implement `build_nsw`, `build_hnsw`, and `build_ivf_pq`. Clone the immutable `Dataset` for each build so every index sees
the same row order and vector bytes. Keep metric and configuration creation outside the corresponding timer.

### Checkpoint 2: Balance Warm-up and Timed Search

Implement `warm_up` and `measure` with the same cyclic order. Propagate every search error instead of skipping a query.
Record one result set and one duration per query for each index, while placing only the `search` call inside the timed
region.

Warm-up reduces one-time effects, but it does not make this small one-process benchmark statistically rigorous.

### Checkpoint 3: Select Nearest-rank Percentiles

Implement `percentile` for a nonempty sorted duration slice and `0 <= percent <= 100`. Use the rule above; do not use a
floor fraction of `len - 1`.

Run the focused example tests:

```sh
cd rust
cargo test -p vector-core-starter --example recall
```

The tests pin the workload domains, exact configuration matrix, constructor-only build timers, balanced cyclic order,
arithmetic-mean recall, nearest-rank percentiles, five-row report, and IVF-PQ accounting.

## Read the Six Output Lines

Run the completed benchmark:

```sh
cargo run --release -p vector-core-starter --example recall
```

It prints exactly five comparison rows in this order:

```text
flat
ivf_flat
nsw
hnsw
ivf_pq
```

Every row uses the same schema:

```text
{name}: rows=2000, dimensions=16, queries=100, metric=euclidean, k=10, build_ms={:.2}, recall={:.3}, p50_us={:.1}, p99_us={:.1}
```

The sixth line is deterministic representation accounting:

```text
ivf_pq search representation: codes_bytes=8000, codebooks_bytes=1024, search_bytes=9024, full_vectors_bytes=128000, compression=14.2x
```

`search_bytes` is `codes_bytes + codebooks_bytes`. The `14.2x` ratio compares full vector component bytes with that PQ
search representation. It is not total-memory compression: the index also retains full vectors for reranking, coarse
centroids, row IDs, list containers, and other allocations.

Confirm these invariant properties:

- all five rows describe the same workload, Euclidean metric, and `k`;
- Flat recall is exactly `1.000`;
- every recall is finite and in `[0, 1]`; and
- p99 is not below p50.

Do not turn the observed durations, approximate recall values, or row ordering into a general performance claim.

## Chapter 6 Review

After the release benchmark completes, explain:

- why all indexes must share data, queries, metric, and `k`;
- why Flat ground truth is computed before approximate measurements;
- what belongs inside and outside build and search timers;
- why warm-up and timed passes use the same balanced cyclic order;
- why recall is averaged across all query result sets;
- how nearest-rank p50 and p99 are selected; and
- which bytes the IVF-PQ accounting includes and excludes.

Keep this checkpoint focused on one reproducible local comparison. External datasets, parameter sweeps, resident-memory
measurement, multiple benchmark processes, and publication-quality confidence intervals are useful extensions rather
than requirements.

{{#include copyright.md}}
