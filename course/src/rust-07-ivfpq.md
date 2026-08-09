# Compress IVFFlat with Product Quantization

> **Optional Chapter 6**
>
> Complete [Benchmark Recall and Latency](./rust-06-benchmark.md) first. Finish with a residual IVF-PQ index that uses
> compact codes for candidate scoring, reranks a shortlist with full vectors, and measures compression together with
> recall and latency.

IVFFlat avoids comparing a query with every row, but it still reads every component of each vector in the probed lists.
For a 128-dimensional `f32` vector, that is 512 bytes of vector data per candidate. Product quantization replaces that
search representation with a short sequence of learned codeword IDs.

This optional chapter follows the product-quantization design introduced by
[Jégou, Douze, and Schmid](https://doi.org/10.1109/TPAMI.2010.57). It combines a coarse IVF partition with product
quantization of the residual, the structure commonly called IVFADC or IVF-PQ. The
[Faiss index guide](https://github.com/facebookresearch/faiss/wiki/Faiss-indexes#summary-of-methods) uses the same
coarse-quantizer-plus-residual-PQ decomposition.

## Replace One Vector with Several Small Choices

Suppose a vector has eight dimensions. Split it into two four-dimensional subvectors:

```text
residual = [ 0.7, -0.1, 0.3, 0.2 | -0.4, 0.8, 0.1, -0.2 ]
             subvector 0              subvector 1
```

Train a separate codebook for each subvector position. If each codebook has four codewords, encoding chooses the nearest
codeword from each side:

```text
subvector 0 -> codeword 2
subvector 1 -> codeword 0
PQ code     -> [2, 0]
```

The two IDs describe one point in the Cartesian product of the two codebooks. The course stores each ID as a `u8`, so an
encoded vector uses one byte per subquantizer even when the codebook has fewer than 256 entries.

The supplied configuration uses 128 dimensions and 16 subquantizers. One full vector occupies `128 * 4 = 512` bytes,
while its PQ code occupies 16 bytes. Shared codebooks add a fixed cost rather than a per-row cost.

## Quantize the Residual, Not the Original Vector

IVF already gives every row a coarse centroid `c`. For a stored vector `x`, encode the residual:

```text
r = x - c
```

Rows in one list are already near the same coarse centroid. Their residuals cover a smaller local region than the
original vectors, which gives the PQ codebooks a more focused quantity to approximate.

Keep the two kinds of learned values distinct:

- an **IVF centroid** chooses the inverted list;
- a **PQ codeword** approximates one subvector of the residual inside that list.

Train one PQ codebook per subvector position across all row residuals. Each codebook uses seeded k-means, just as the
coarse IVF build does. Equal data and configuration must produce equal coarse centroids, PQ codebooks, list sizes, and
search results.

## Score Codes with Lookup Tables

Do not reconstruct every candidate vector during the approximate scan. For each probed list, subtract its coarse
centroid from the query and build one distance table per subquantizer:

```text
table[m][j] = squared_l2((query - coarse_centroid)[m], codebook[m][j])
```

A row's approximate squared distance is then a sum of lookups:

```text
distance(code) = table[0][code[0]] + ... + table[M - 1][code[M - 1]]
```

The query remains full precision. Only stored row residuals are quantized, so this is asymmetric distance computation.
Squared Euclidean distance preserves the same ordering as Euclidean distance and avoids a square root for every
approximate candidate.

**Prediction:** With 16 subquantizers and 16 codewords per codebook, how many table entries does one probed list need?
How many additions score one encoded row after those tables exist?

## Rerank Before Returning

Quantized distance selects a shortlist; it does not define the final distance. Keep the best `rerank` row offsets under
the lookup-table score, then compute exact Euclidean distance from the original dataset for those rows and retain the
final `k`.

```text
probed lists -> PQ distance -> rerank-row shortlist -> exact distance -> top-k
```

The implementation uses at least `k` shortlist entries even when `rerank < k`, so it can return up to `k` rows. A larger
rerank budget gives exact scoring a chance to recover rows whose PQ approximation was imprecise, but it reads more full
vectors.

The base `Dataset` remains available for this exact stage. Therefore the reported compression covers the PQ search
representation—codes plus shared PQ codebooks—not total process memory or the base table. Removing full vectors would
also remove exact reranking and is outside this chapter.

## Build IVF-PQ in Rust

You will modify:

```text
rust/vector-starter/core/src/pq.rs
```

The starter already exposes `IvfPqConfig`, `IvfPqIndex`, the `VectorIndex` implementation, byte-accounting methods, and a
release benchmark. Keep public APIs, existing indexes, and tests unchanged.

All commands in this chapter exercise the cumulative starter workspace. Complete Chapters 1–5 first; an untouched
starter stops at an earlier chapter's `todo!()` implementation.

The optional configuration is:

| Field | Meaning | Supplied value |
| --- | --- | ---: |
| `partitions` | Coarse IVF lists | 64 |
| `probes` | Lists visited per query | 8 |
| `iterations` | Seeded k-means rounds for both levels | 12 |
| `subquantizers` | Equal residual slices | 16 |
| `codebook_size` | Codewords per slice | 16 |
| `rerank` | Full-precision shortlist budget | 100 |
| `seed` | Reproducible training seed | 7 |

## What Must Hold, and What Breaks If It Doesn't

This implementation accepts only `Metric::Euclidean` and rejects another metric
before training. If that validation were removed, metric-specific coarse
assignment would be mixed with squared-L2 PQ tables and the scores would no
longer describe one ranking.

Your IVF configuration must satisfy `1 <= probes <= partitions <= rows`
and `iterations > 0`.

The code layout must have `subquantizers > 0`, dimension divisible by
`subquantizers`, and `2 <= codebook_size <= min(256, rows)`. A
non-divisible dimension is rejected. If it were accepted, equal slices would
omit residual components rather than leave residual bytes.

After encoding, every row must appear in exactly one IVF list with
exactly `subquantizers` codes.

Each code must be a valid index into its subquantizer's codebook.

Training, encoding, and query tables must all subtract the same coarse
centroid. Subtracting different centroids produces residuals from
different reference points.

Returned distances must be recomputed from original vectors, remain
representable as finite `f32`, and follow the public `(distance, row)`
ordering.

Compressed byte counts must include codes and PQ codebooks but exclude
row IDs, coarse centroids, allocator overhead, and the base dataset
retained for reranking.

## Checkpoint 1: Validate and Build the Coarse Lists

Implement `IvfPqIndex::try_new`. Validate the configuration before training. Call the existing `IvfFlatIndex::try_new`
with the same partitions, probes, iterations, and seed, then copy its final coarse centroids.

Assign every dataset row to its nearest final centroid and compute `row - centroid`. Rebuilding these assignments from
the final centroids preserves the same complete-list invariant as Chapter 2.

The Euclidean-only boundary is deliberate. Cosine and inner-product quantization require additional representation and
distance choices; returning plausible numbers would not establish a correct metric contract.

Run the invalid-metric and subvector-layout cases:

```sh
cd rust
cargo test -p vector-core-starter --test indexes ivf_pq_validates_its_euclidean_code_layout
```

## Checkpoint 2: Train One Codebook per Subvector

Split every residual into `subquantizers` contiguous slices of equal length. For each slice position:

1. choose `codebook_size` distinct seeded residual rows;
2. copy that slice from each selected row as an initial codeword;
3. assign every residual slice to its nearest codeword under squared Euclidean distance;
4. replace each non-empty codeword with the component-wise mean of its assignments; and
5. stop after convergence or `iterations` rounds.

Keep a codeword unchanged when its cluster is empty. Reuse the starter's `DeterministicRng` from `src/search.rs`, as in
Chapter 2, to choose distinct seeded rows. Derive a different deterministic seed for each subquantizer so all codebooks
do not begin with the same row sequence.

After training, encode each row by choosing one codeword per residual slice. Store the resulting `u8` IDs with the row in
its IVF list.

```sh
cargo test -p vector-core-starter --test indexes ivf_pq_build_is_seeded_and_codes_each_row
```

This test checks deterministic coarse centroids and codebooks, complete list membership, one-byte codes, and the stated
byte accounting.

## Checkpoint 3: Scan Codes and Rerank

Implement `search_with_probes`:

1. validate the query, probe count, and nonzero rerank budget;
2. rank coarse centroids and visit the nearest `probes` lists;
3. build residual-distance lookup tables for each visited list;
4. sum one table entry per code to retain an approximate shortlist;
5. compute exact Euclidean distances for the shortlist row offsets; and
6. return the exact top-k in deterministic order.

Keep coarse-centroid distances, lookup-table scores, and exact rerank distances in `f64`. At the public `Neighbor`
boundary, discard exact distances that are non-finite or greater than `f32::MAX`, convert the remaining distances to
`f32`, and select and sort results by `(distance, row)`. This public ordering also resolves two distinct `f64` distances
that round to the same `f32`. If discarded candidates leave fewer than `min(k, rows)` results, return an error. A row
offset must stay attached to its code through both heaps.

Probe every list and rerank every row as an exactness boundary:

```sh
cargo test -p vector-core-starter --test indexes ivf_pq_full_scan_and_rerank_matches_exact_search
```

When both budgets cover the dataset, the PQ score changes only the order in which rows enter exact reranking. The final
result must match `FlatIndex`.

Run the complete IVF-PQ core group, including the large-finite ordering, public tie-break, and result-representability
cases:

```sh
cargo test -p vector-core-starter --test indexes ivf_pq_
```

The supplied `IndexConfig::IvfPq` variant also lets the unchanged Chapter 1 adapter reach the completed index for a
Euclidean SQL query:

```sh
cargo test -p vector-datafusion-starter --test sql ivf_pq_is_visible_in_explain
```

This check requires no DataFusion edits. It verifies that the physical plan names `index=ivf_pq` while preserving the
same safe matcher and final bounded sort.

## Checkpoint 4: Compare Compression, Recall, and Latency

Run the completed optional benchmark:

```sh
cargo run --release -p vector-core-starter --example quantization
```

The deterministic workload uses 5,000 rows, 128 dimensions, 100 queries, Euclidean distance, and `k = 10`. It prints one
IVFFlat row, one IVF-PQ row, and the compressed search-representation accounting.

With the supplied shapes, the exact byte values are:

```text
codes_bytes         = 5,000 * 16               = 80,000
codebooks_bytes     = 16 * 16 * 8 * 4          = 8,192
full_vectors_bytes  = 5,000 * 128 * 4           = 2,560,000
compression         = 2,560,000 / 88,192        = 29.0x
```

Build time and latency vary by machine. Recall is deterministic for the supplied seed. Do not require IVF-PQ to be
universally faster: this implementation favors readable scalar loops, and lookup-table setup can dominate small
workloads.

Temporarily change the `rerank` field in the `IvfPqConfig` constructed by
`rust/vector-starter/core/examples/quantization.rs` to 20, 100, and 200, rerunning the command after each change. Restore
the supplied value of 100 before you finish, and leave the data, query set, coarse IVF configuration, PQ layout, metric,
and `k` unchanged. Explain how recall and query latency respond. The encoded-byte count should not change because
reranking changes query work, not stored codes.

## Review Your Optional Chapter Result

After all IVF-PQ core tests, the DataFusion plan check, and the release benchmark pass, explain:

- why IVF centroids and PQ codewords solve different parts of the search;
- why residuals use the selected row or query list's coarse centroid;
- how asymmetric lookup tables avoid reconstructing every candidate;
- why reranking needs the original vectors;
- which bytes the reported compression includes and excludes; and
- why a compressed representation does not guarantee a speedup on every workload or implementation.

Keep this follow-up focused on an executable IVF-PQ mental model. Bit-packed codes, cosine or inner-product support,
optimized product quantization, SIMD table scans, persistent layouts, training samples separate from indexed rows, and
removing full vectors from memory remain outside its scope.

{{#include copyright.md}}
