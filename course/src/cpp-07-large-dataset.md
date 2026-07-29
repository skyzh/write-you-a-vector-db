# Benchmarking IVFFlat and HNSW on SIFT1M

{{#include cpp-deprecation.md}}

<div class="warning">

**Optional capstone:** Use this chapter after completing IVFFlat or a private HNSW implementation. The benchmark shows how
index parameters affect query speed and recall on a larger data set; the earlier chapter checks remain the place to debug
the algorithms themselves.

</div>

<div class="info">

**Benchmark credit:** The SIFT1M benchmark harness was contributed by
[UnpureRationalist](https://github.com/UnpureRationalist) in
[bustub-vectordb PR #2](https://github.com/skyzh/bustub-vectordb/pull/2).

</div>

Small SQL fixtures expose correctness bugs, but they do not show how an approximate index behaves at realistic scale.
This capstone uses the standard SIFT1M corpus to connect three quantities:

- the time spent loading data and preparing the index;
- end-to-end query throughput; and
- the probability that the exact nearest neighbor appears near the top of an approximate result.

The benchmark harness is `tools/vectordb_bench/vectordb_bench.cpp`. Its provided configuration uses HNSW; a later section
shows the small control-flow change needed to benchmark IVFFlat.

## What the Harness Measures

On each run, `bustub-vectordb-bench`:

1. creates `t1(v1 VECTOR(128), v2 INTEGER)`;
2. creates an L2 HNSW index with `m = 16`, `ef_construction = 64`, and `ef_search = 100`;
3. reads one million base vectors and inserts each through an SQL statement;
4. reads 10,000 query vectors and their exact ground-truth neighbors;
5. asks BusTub for 100 rows per query; and
6. reports cumulative timestamps and `R@1`, `R@10`, and `R@100`.

The index exists before the first row is inserted. “Loading database” therefore includes SQL construction and parsing,
table insertion, and incremental HNSW maintenance. It is not a pure bulk-index-build timer.

Queries also run one at a time through the full SQL path. Their elapsed time includes SQL parsing, planning, index
execution, tuple materialization, result conversion, and metric bookkeeping. Treat this as an end-to-end BusTub
measurement, not as the latency of the HNSW search function by itself.

The harness logs failed inserts, but it does not check each query's `ExecuteSql` return value. If a run produces empty
results, check query execution before tuning the index.

### What `R@R` Means

For each query, SIFT1M supplies the exact nearest vector as the first ground-truth ID. The harness checks whether that one
ID appears within the first 1, 10, or 100 approximate results. This is **1-nearest-neighbor recall at rank R**, matching
the convention used by [Faiss's SIFT1M experiments](https://github.com/facebookresearch/faiss/wiki/Indexing-1M-vectors).
It is not the fraction of the exact top 100 set that BusTub recovered.

The recall values should always follow:

```text
0 <= R@1 <= R@10 <= R@100 <= 1
```

A result can have low `R@1` and high `R@100`: the correct neighbor was found, but ranked behind other candidates.

## Build an Optimized Benchmark

Create a Release build for the benchmark. On Ubuntu, from the repository root:

```shell
cmake -S . -B build-bench \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -DCMAKE_C_COMPILER=clang-14 \
  -DCMAKE_CXX_COMPILER=clang++-14
cmake --build build-bench --target vectordb-bench -j8
```

On macOS, use the `llvm@14` compiler paths from the overview instead. The executable is
`build-bench/bin/bustub-vectordb-bench`.

## Prepare SIFT1M

Download ANN_SIFT1M from the [TexMex corpus page](http://corpus-texmex.irisa.fr/), which is also the source named by the
[Faiss benchmark documentation](https://github.com/facebookresearch/faiss/blob/main/INSTALL.md). The course does not
redistribute the dataset. The benchmark's `FvecsRead` and `IvecsRead` functions read the corpus files directly; no
conversion script or separate reader is required. Extract or copy the files into this layout:

```text
build-bench/
  sift1M/
    sift_base.fvecs
    sift_query.fvecs
    sift_groundtruth.ivecs
```

The harness does not use `sift_learn.fvecs`. Check the required paths before starting:

```shell
test -f build-bench/sift1M/sift_base.fvecs
test -f build-bench/sift1M/sift_query.fvecs
test -f build-bench/sift1M/sift_groundtruth.ivecs
```

SIFT1M contains one million 128-dimensional base vectors and 10,000 queries. BusTub stores vectors as doubles in both the
table and the graph, keeps table pages in memory, and allocates a large buffer pool in the harness. Plan for several
gigabytes of available memory. The million individual SQL inserts can also take substantial time.

## Run the HNSW Benchmark

Run from the directory that directly contains `sift1M`:

```shell
cd build-bench
./bin/bustub-vectordb-bench | tee run.txt
cd ..
```

All timestamps are seconds since process start. Use the timestamp printed beside `Loading queries` as the end of the
base-row load and incremental graph build. Use the difference between `Doing query, #0` and `Compute recalls` as the
query duration:

```text
query_seconds = compute_recalls_timestamp - first_query_timestamp
queries_per_second = 10000 / query_seconds
```

## Run an IVFFlat Benchmark

The provided harness creates its HNSW index before loading rows, so each insert incrementally updates the graph. IVFFlat
has a different build path: it learns centroids from data already in the table. To benchmark IVFFlat in your private
working copy:

1. keep the base-vector insertion loop in `InsertIndexVectorData`;
2. move index creation after that loop; and
3. replace the HNSW statement with an IVFFlat statement such as:

```sql
CREATE INDEX t1v1ivfflat ON t1 USING ivfflat
  (v1 vector_l2_ops) WITH (lists = 10, probe_lists = 3);
```

Add timestamps immediately before and after `CREATE INDEX` if you want to separate table loading from the offline
IVFFlat build. The query reader, ground-truth reader, and recall calculation can stay unchanged.

## Explore One Tradeoff

### HNSW

The index SQL is the `create_index` string in `vectordb_bench.cpp`. Keep `m = 16` and
`ef_construction = 64` fixed, then compare `ef_search = 100` and `ef_search = 200`.

Each benchmark query uses `LIMIT 100`, so `k = 100`. The lookup contract from the HNSW chapter searches with width
`max(k, ef_search)`. As a result, `ef_search = 50` and `ef_search = 100` both produce an effective width of 100; comparing
100 with 200 actually changes the number of candidates the graph search may retain.

Rebuild and rerun after changing the string. For a controlled comparison, use the same random seed in your private HNSW
implementation; otherwise repeat each configuration and report the variation.

### IVFFlat

Keep `lists` fixed and change `probe_lists`, for example from 1 to 3. The first run searches one centroid list per query;
the second searches three. This directly exposes the IVFFlat tradeoff between scanning more candidates and finding more
of the exact neighbors.

The following compact table is enough to compare the runs:

| Index | Parameters | Preparation (s) | Query (s) | QPS | `R@1` | `R@10` | `R@100` |
|---|---|---:|---:|---:|---:|---:|---:|
| HNSW | `m=16, ef_construction=64, ef_search=100` |  |  |  |  |  |  |
| HNSW | `m=16, ef_construction=64, ef_search=200` |  |  |  |  |  |  |
| IVFFlat | `lists=10, probe_lists=1` |  |  |  |  |  |  |
| IVFFlat | `lists=10, probe_lists=3` |  |  |  |  |  |  |

For HNSW, preparation includes row insertion and incremental graph maintenance. For IVFFlat, it includes row insertion
followed by the offline centroid build, so the preparation column represents the complete path to a queryable index in
both cases.

## Reading the Results

The benchmark is most useful as a comparison rather than a pass/fail exercise. Keep the command output and the table,
then add a short explanation of what changed when you increased `ef_search` or `probe_lists`. If `R@10` or `R@100` rises
while `R@1` stays flat, the exact neighbor is appearing in the candidate set without consistently ranking first. If the
numbers do not change at all, trace the parameter from the SQL option into the index lookup before drawing a conclusion.

Keep private HNSW implementation changes under the same academic-integrity rule as the previous chapters.

{{#include copyright.md}}
