# Try the Vector Database from SQL

{{#include rust-in-progress.md}}

Before you implement the table adapter or optimizer, use the supplied system once. You will run one nearest-neighbor query,
create one vector index, and see the physical plan change while the SQL result stays the same.

This tour uses the completed `vector-datafusion` example. You do not need to read or modify its source. Your own work begins
in Chapter 1.

## Launch the Supplied Shell

For an interactive run, start from the `rust/` directory:

```sh
cargo run -p vector-datafusion --example sql
```

The shell creates and registers a small in-memory table when it starts:

```text
points(id UInt64, payload Utf8, embedding FixedSizeList<Float32, 3>)
```

It then accepts one SQL statement per input line. For a repeatable first run from the repository root, paste the whole
transcript below into your terminal instead of entering the statements interactively:

```sh
cd rust
cargo run -p vector-datafusion --example sql <<'SQL'
EXPLAIN SELECT id, payload FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3
SELECT id, payload FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3
CREATE INDEX points_embedding_idx ON points USING ivfflat (embedding)
EXPLAIN SELECT id, payload FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3
SELECT id, payload FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3
SQL
```

## Observe the Stable Query and Changing Plan

Before the index exists, DataFusion reads the ordinary in-memory table:

```text
SortExec: TopK(fetch=3), ...
  DataSourceExec: partitions=1, ...
```

The first query returns these rows:

```text
1  one
2  two
3  three
```

The `CREATE INDEX` statement builds the course's fixed cosine IVFFlat index and attaches it to `points.embedding`. The
second `EXPLAIN` reaches the course-owned scan:

```text
SortExec: TopK(fetch=3), ...
  VectorIndexScanExec: index=ivf_flat, metric=Cosine, query_dim=3, fetch=Some(3), ordered=false
```

The second query is byte-for-byte the same SQL and returns the same three rows. The index changes how candidates reach
DataFusion's final sort; it does not change the query contract.

## Know What This Command Means

DataFusion parses and logically plans `CREATE INDEX`, but the pinned version does not provide a physical executor that can
build this course's index. The supplied shell therefore owns exactly one command:

```sql
CREATE INDEX points_embedding_idx ON points USING ivfflat (embedding)
```

It accepts that fixed index name, table, column, and index kind once per shell session. It does not implement an index
catalog, persistence, `DROP INDEX`, arbitrary table or column selection, options, or general DDL semantics.

That narrow boundary keeps the first experience concrete without turning the course into a parser or catalog project.
Next, [Chapter 1](./rust-02-datafusion.md) opens the path you just used: you will build the Arrow table, attach one selected
vector field, and make the optimizer choose `VectorIndexScanExec` only when doing so is safe.

{{#include copyright.md}}
