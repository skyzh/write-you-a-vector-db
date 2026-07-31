# Benchmark Recall and Latency

> **Chapter 5**
>
> Complete [Add Hierarchy with HNSW](./rust-05-hnsw.md) first. Finish with one release-mode benchmark that compares
> Flat, IVFFlat, NSW, and HNSW on the same data, queries, metric, and top-k contract.

An approximate index is useful only when its speed is attached to a result-quality measurement. A latency number by
itself cannot tell you whether an index found the right neighbors, and a recall number from a different workload cannot
explain the latency you measured.

This chapter gives every index the same immutable dataset and query vectors. Exact Flat search produces the expected
top-k row offsets. Each approximate result is compared with those offsets using recall@k, while build time and query
latency are measured separately.

## Compare One Workload

The supplied example uses a small deterministic workload:

```text
rows        2,000
dimensions  16
queries     100
metric      cosine
k           10
```

The generated values are repeatable, so equal code and configuration produce the same rows, queries, and recall. The
fixture is large enough to exercise all four implementations quickly, but it is not a claim about production embedding
distributions or million-row performance.

Keep the workload fixed while comparing these index configurations:

| Index | Configuration |
| --- | --- |
| Flat | Exact search over every row |
| IVFFlat | 32 partitions, 6 probes, 12 iterations, seed 7 |
| NSW | 12 connections, construction width 64, search width 40 |
| HNSW | The NSW budgets plus maximum level 12 and seed 7 |

Changing several index budgets and the workload at once makes the result hard to explain. First produce this shared
baseline. Parameter sweeps can come afterward.

## Separate Correctness from Timing

The benchmark follows this order:

1. build the exact index and compute exact top-k results for every query;
2. build each index separately and record only its construction time;
3. run one untimed pass over all queries to warm the code and data paths;
4. time each query individually in release mode;
5. compare every returned row set with the saved exact result; and
6. report average recall together with p50 and p99 query latency.

The timed region contains only `index.search(query, k)`. Dataset generation, index construction, result comparison, and
printing stay outside it. `black_box` keeps each returned result observable during both warm-up and measurement.

**Prediction:** Flat search should report recall `1.000`. If it does not, which part of the benchmark contract is broken:
the approximate index, the ground truth, or the comparison code?

## Understand the Report

Each output row has the same fields:

```text
name: rows=..., dimensions=..., queries=..., k=...,
build_ms=..., recall=..., p50_us=..., p99_us=...
```

- `build_ms` measures construction once. It is not divided across queries.
- `recall` is the mean recall@10 across the 100 query result sets.
- `p50_us` is the nearest-rank median query latency.
- `p99_us` is the nearest-rank tail latency at the 99th percentile.

Sort the latency samples before selecting a percentile. For `n` samples and percentage `p`, use one-based nearest rank
`ceil(p * n / 100)`, then convert that rank to a zero-based index. Clamp the final index to the last sample.

Timing values will vary across machines and runs. Treat them as observations, not pass/fail thresholds. Recall should be
stable because the workload and seeded builds are deterministic.

## Build the Benchmark in Rust

You will modify:

```text
rust/vector-starter/core/examples/recall.rs
```

The example already generates the workload, builds Flat and IVFFlat, computes recall, records individual query
durations, and prints the report. Complete the four Chapter 5 TODOs without changing index implementations or public APIs.

### Checkpoint 1: Add NSW and HNSW

Implement `build_nsw` and `build_hnsw` with the fixed configurations in the table above. Clone the immutable `Dataset` for
each build so every index sees the same row order and vectors.

Keep the construction timer around only the corresponding builder call. Do not include the earlier exact-search ground
truth or another index's build.

### Checkpoint 2: Warm Each Index

Implement `warm_up`. Search every query once with the same `k` used by the timed pass and send each result through
`std::hint::black_box`. Propagate search errors instead of skipping a query.

Warm-up reduces one-time effects, but it does not make this small benchmark statistically rigorous. Repeat the whole
process when investigating noise.

### Checkpoint 3: Select Latency Percentiles

Implement `percentile` for a nonempty, already sorted duration slice and `0 <= percent <= 100`. Use the nearest-rank rule
above. The benchmark calls it for p50 and p99 after sorting all 100 samples.

**Prediction:** With 100 sorted samples, which zero-based element supplies p99? Work through the one-based rank before
writing the expression.

### Checkpoint 4: Run and Explain the Comparison

Run the completed benchmark from `rust/`:

```sh
cargo run --release -p vector-core-starter --example recall
```

Confirm all four rows appear, Flat recall is exactly `1.000`, every recall lies in `[0, 1]`, and p99 is not below p50.
Do not require one approximate index to be universally faster or more accurate than another on this small fixture.

Then choose two rows and explain the observed tradeoff. Name the build budget, search budget, recall, and latency fields
that support your explanation instead of inferring a general ranking from the index names.

## Review Your Chapter 5 Result

After the release benchmark completes, explain:

- why all indexes must share data, queries, metric, and `k`;
- why exact results are computed before approximate measurements;
- what belongs inside and outside the timed query region;
- why warm-up is untimed;
- how nearest-rank p50 and p99 are selected; and
- which conclusions this deterministic 2,000-row fixture cannot support.

Keep this checkpoint focused on a reproducible local comparison. Loading SIFT1M or another external dataset, measuring
resident memory, running many benchmark processes, and producing publication-quality confidence intervals are useful
extensions rather than requirements for this chapter.

{{#include copyright.md}}
