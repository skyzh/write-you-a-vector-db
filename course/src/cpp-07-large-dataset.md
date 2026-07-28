# Benchmarking HNSW on SIFT1M

{{#include cpp-deprecation.md}}

<div class="warning">

**Optional capstone:** This benchmark requires a completed HNSW implementation in your private repository. The harness
measures observable query behavior, but it cannot prove that your index built multiple layers or used them during lookup.
Complete the structural checks in the previous chapter before interpreting benchmark numbers.

</div>

Small SQL fixtures expose correctness bugs, but they do not show how an approximate index behaves at realistic scale.
This capstone uses the standard SIFT1M corpus to connect three quantities:

- the time spent loading rows and maintaining the HNSW graph;
- end-to-end query throughput; and
- the probability that the exact nearest neighbor appears near the top of an approximate result.

The benchmark harness is `tools/vectordb_bench/vectordb_bench.cpp`.

## Understand the Harness

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

The harness logs failed inserts, but it does not check the return status of each query. Very low recall can therefore mean
either poor approximate search or a query-execution failure. The smoke fixture and the earlier SQL regression are required
preflights, not optional warm-ups.

### What `R@R` Means

For each query, SIFT1M supplies the exact nearest vector as the first ground-truth ID. The harness checks whether that one
ID appears within the first 1, 10, or 100 approximate results. This is **1-nearest-neighbor recall at rank R**, matching
the convention used by [Faiss's SIFT1M experiments](https://github.com/facebookresearch/faiss/wiki/Indexing-1M-vectors).
It is not the fraction of the exact top 100 set that BusTub recovered.

Every valid run must satisfy:

```text
0 <= R@1 <= R@10 <= R@100 <= 1
```

A result can have low `R@1` and high `R@100`: the correct neighbor was found, but ranked behind other candidates.

## Preflight the Storage Fix

A 128-dimensional vector row occupies 1,036 serialized bytes in this starter. The benchmark snapshot fixes an unsigned
offset underflow that previously wrote the fourth such tuple beyond its table page. Before spending time on SIFT1M, run
the focused regression from the repository root:

```shell
cmake --build build --target table_page_test -j8
./build/test/table_page_test
```

The test must pass under the Debug configuration from the course setup. That configuration enables AddressSanitizer.

## Build an Optimized Benchmark

Debug sanitizers distort timings and consume additional memory. Keep the Debug build for correctness checks and create a
separate Release build for the benchmark. On Ubuntu, from the repository root:

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

## Run a Small Fixture First

The executable resolves `sift1M/*` relative to its working directory. Generate a disposable eight-vector fixture under
`build-bench/smoke`:

```shell
python3 - <<'PY'
from pathlib import Path
import struct

root = Path("build-bench/smoke/sift1M")
root.mkdir(parents=True, exist_ok=True)

def write_fvecs(path, rows):
    with path.open("wb") as file:
        for row in rows:
            file.write(struct.pack("<i", len(row)))
            file.write(struct.pack(f"<{len(row)}f", *row))

def write_ivecs(path, rows):
    with path.open("wb") as file:
        for row in rows:
            file.write(struct.pack("<i", len(row)))
            file.write(struct.pack(f"<{len(row)}i", *row))

write_fvecs(root / "sift_base.fvecs", [[float(i)] * 128 for i in range(8)])
write_fvecs(root / "sift_query.fvecs", [[0.0] * 128])
write_ivecs(root / "sift_groundtruth.ivecs", [list(range(8)) + [0] * 92])
PY

cd build-bench/smoke
../bin/bustub-vectordb-bench
cd ../..
```

For this fixture, vector 0 is the exact match. A correct completed implementation should report:

```text
R@1 = 1.0000
R@10 = 1.0000
R@100 = 1.0000
```

If the program reports an insert failure, crashes, returns no rows, or misses vector 0, stop here. A million-row run will
only make the same problem slower to diagnose.

## Prepare SIFT1M

Download ANN_SIFT1M from the [TexMex corpus page](http://corpus-texmex.irisa.fr/), which is also the source named by the
[Faiss benchmark documentation](https://github.com/facebookresearch/faiss/blob/main/INSTALL.md). The course does not
redistribute the dataset. Extract or copy its files into this layout:

```text
build-bench/
  full/
    sift1M/
      sift_base.fvecs
      sift_query.fvecs
      sift_groundtruth.ivecs
```

The harness does not use `sift_learn.fvecs`. Check the required paths before starting:

```shell
test -f build-bench/full/sift1M/sift_base.fvecs
test -f build-bench/full/sift1M/sift_query.fvecs
test -f build-bench/full/sift1M/sift_groundtruth.ivecs
```

SIFT1M contains one million 128-dimensional base vectors and 10,000 queries. BusTub stores vectors as doubles in both the
table and the graph, keeps table pages in memory, and allocates a large buffer pool in the harness. Plan for several
gigabytes of available memory. The million individual SQL inserts can also take substantial time.

## Run and Interpret the Benchmark

Run from the directory that directly contains `sift1M`:

```shell
cd build-bench/full
../bin/bustub-vectordb-bench | tee run.txt
cd ../..
```

All timestamps are seconds since process start. Use the timestamp printed beside `Loading queries` as the end of the
base-row load and incremental graph build. Use the difference between `Doing query, #0` and `Compute recalls` as the
query duration:

```text
query_seconds = compute_recalls_timestamp - first_query_timestamp
queries_per_second = 10000 / query_seconds
```

Do not compare raw timings across machines without recording the CPU, available memory, compiler, build type, and commit.
Do not use the contributor's example output as a correctness threshold; recall depends on your HNSW implementation,
random graph construction, and search parameters.

## Measure One Tradeoff

The index SQL is the `create_index` string in `vectordb_bench.cpp`. Keep `m = 16` and
`ef_construction = 64` fixed, then compare at least `ef_search = 100` and `ef_search = 200`. Values below 100 may
not change a correct implementation because every benchmark query requests 100 rows.

Rebuild and rerun after changing the string. For a controlled comparison, use the same random seed in your private HNSW
implementation; otherwise repeat each configuration and report the variation.

Record enough context to reproduce the result:

| Commit | CPU / RAM | Build | `m` | `ef_construction` | `ef_search` | Load + build (s) | Query (s) | QPS | `R@1` | `R@10` | `R@100` |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `...` | `...` | Release | 16 | 64 | 100 |  |  |  |  |  |  |
| `...` | `...` | Release | 16 | 64 | 200 |  |  |  |  |  |  |

Larger search beams usually exchange more work for better recall, but do not force the numbers to match that story. A
flat or worse result is evidence to inspect candidate ordering, entry-point descent, layer membership, and whether
`ef_search_` actually reaches `SearchLayer`.

## Completion Checkpoint

You are done with this optional capstone when:

- the table-page regression and eight-vector smoke fixture pass;
- the complete SIFT1M run finishes without failed inserts or crashes;
- the three recall values are in range and nondecreasing;
- your report identifies the exact code, build, machine, parameters, load/build time, query time, and QPS; and
- you can explain why the load timer includes graph maintenance, why this metric is not top-100 set recall, and what
  changed when you increased `ef_search`.

Keep HNSW implementation changes private under the same academic-integrity rule as the previous chapters.

{{#include copyright.md}}
