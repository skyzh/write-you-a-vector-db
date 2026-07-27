# Navigate a Proximity Graph with NSW

> **Chapter ID:** `VDB-NSW`
>
> **Prerequisite:** `VDB-EVAL`
>
> **Status:** executable reference preview; human review unrecorded

Before this chapter, candidates come from partitions. After it, insertion
builds a bounded-degree proximity graph and query uses a best-first beam to
navigate toward nearby points.

## Contract

Relevant code is `rust/vector-core/src/graph.rs` and `nsw.rs`.

1. **I1 — Visit once:** one layer search computes each visited row's distance
   at most once.
2. **I2 — Two frontiers:** the candidate queue expands the nearest unvisited
   option while a bounded result heap tracks the worst accepted point.
3. **I3 — Stop rule:** traversal stops only when the nearest pending candidate
   is worse than the full result frontier's worst member.
4. **I4 — Degree:** pruning leaves at most `max_connections` outgoing neighbors
   per row, with deterministic distance-and-row ordering.

`ef_construction` controls insertion effort and `ef_search` controls query
effort. They are budgets, not correctness proofs. The required lifecycle is
bulk build followed by read-only queries.

## Checkpoints

1. Implement best-first search over a supplied adjacency list.
2. Insert rows one at a time using the existing graph as the candidate source.
3. Add reciprocal connections and deterministic pruning.
4. Query with `ef_search.max(k)` and measure recall against exact search.

## Verification

Run:

```sh
cargo test -p vector-core nsw_high_ef_matches_exact_search_on_connected_fixture
```

The fixture verifies degree bounds and high-budget recall on a connected graph;
it does not prove recall for arbitrary distributions. Stop here rather than
adding hierarchy, deletion, concurrency, or sophisticated diversification
heuristics.

Explain back why one heap cannot serve both traversal roles, which comparison
controls early stopping, and how aggressive pruning can isolate useful paths.
