# Add Hierarchy with HNSW

> **Chapter 4**
>
> Complete [Navigate a Proximity Graph with NSW](./rust-04-nsw.md) first. Finish with seeded sparse graph layers,
> greedy upper-layer descent, layer-zero beam search, and the same SQL top-k running through `index=hnsw`.

The previous chapter searches one NSW graph containing every vector. HNSW adds sparse graph layers above it, much like a
skip list or a mipmap: upper layers make long jumps across the collection, while the complete layer zero refines the
search around the query.

## How Hierarchy Works

Layer zero contains every vector. Each higher layer contains a progressively smaller random subset. A vertex that appears
in an upper layer also appears in every layer below it.

![Sparse HNSW layers route into the complete layer-zero graph](./vector-db/06-hnsw-architecture.svg)

The diagram repeats the same vector IDs across nested layers. Sparse upper-layer edges cross large parts of the dataset;
denser lower-layer edges make shorter moves. The highest promoted vertex becomes the entry point for the whole index.

### Look Up a Query

Begin at the entry point in the highest layer. Greedily follow closer neighbors until no adjacent vertex improves the
distance, then carry that vertex down as the entry point for the next layer.

![HNSW descends through progressively denser layers](./vector-db/06-hnsw-explore.svg)

Upper layers use a width-one greedy walk because their job is coarse routing. At layer zero, reuse the NSW best-first
search from Chapter 3 with width `max(k, ef_search)`, then return the nearest `k` candidates.

```text
entry = top entry point
for level from highest down to 1:
    entry = greedy_search(layer[level], query, entry)

candidates = search_layer(
    layer[0],
    query,
    entry_points=[entry],
    width=max(k, ef_search),
)
return nearest k candidates
```

**Prediction:** Why would using the full beam width in every sparse upper layer do more work without changing the final
result contract? Which layer still needs multiple candidates to produce top-k output?

### Insert a Vector

Assign each new vector a random maximum level. The course uses repeated seeded coin flips: every successful flip promotes
the vector one layer higher, up to `max_level`. This produces many layer-zero vertices and progressively fewer vertices in
higher layers.

Suppose the new vector reaches level one. It belongs to layers one and zero, but not layer two.

![A new vector is promoted to level one and every lower layer](./vector-db/06-hnsw-insert-1.svg)

Start at the current top entry point. Greedily descend through layers above the new vector's level. At each layer the new
vector joins, run the Chapter 3 best-first search with `ef_construction`, choose the nearest `max_connections` candidates,
add reciprocal edges, and prune both sides of rejected edges.

![Search each included layer before connecting the new vector](./vector-db/06-hnsw-insert-2.svg)

```text
target_level = seeded_geometric_level()
entry = top entry point

for level above target_level, from highest down:
    entry = greedy_search(layer[level], new_vector, entry)

for shared level from min(highest, target_level) down to 0:
    candidates = search_layer(layer[level], new_vector, entry, ef_construction)
    connect the nearest max_connections candidates
    prune reciprocal edges to the degree cap
    entry = nearest candidate

if target_level is above the previous highest level:
    add the missing sparse layers
    make the new vector the top entry point
```

When the first vector creates the index, add it to every layer through its sampled level and use it as the entry point.
When a later vector reaches a new highest level, its new upper layers initially contain only that vector.

## Build HNSW in Rust

You will modify:

```text
rust/vector-starter/core/src/graph.rs        greedy_search only
rust/vector-starter/core/src/hnsw.rs
```

The starter already contains the Chapter 3 layer search and pruning interfaces, a deterministic random-number generator,
and the public HNSW configuration and inspection methods. Keep the NSW behavior, metric ordering, public APIs, and
DataFusion matcher unchanged.

### What Must Hold, and What Breaks If It Doesn't

Row `r` must appear in every layer from zero through `levels[r]` and in
no higher layer. A row missing from a lower layer is unreachable from
below; a row in a higher layer than declared violates the level
assignment the construction algorithm depends on.

Given equal data, configuration, and seed, the level sequence and top
layer must be identical. Non-deterministic levels make the graph
structure unreproducible across runs.

Your budget must satisfy `max_connections > 0`,
`ef_construction >= max_connections`, `ef_search > 0`, and `max_level > 0`.

Greedy descent in upper layers must move only to a strictly closer
neighbor and pass exactly one entry point downward. Moving to an
equally-distant neighbor can loop; passing multiple entry points
confuses the layer below's search frontier.

The layer-zero beam search must use the Chapter 3 two-frontier
traversal with width `ef_search.max(k)`. A different search strategy at
layer zero produces results inconsistent with the upper-layer traversal.

Every layer must preserve the degree cap, contain no duplicate or
self-edges, and store every remaining edge at both endpoints.

Every layer stores the same core row offsets used by the dataset and
Arrow batch. Offset drift makes the graph point at the wrong vectors.

### Checkpoint 1: Descend One Layer

Implement `greedy_search` in `graph.rs`. Start from one valid entry point, repeatedly choose its nearest allowed neighbor,
and move only when that neighbor is strictly better than the current vertex. A strict improvement prevents cycles and
makes distance-and-row tie order deterministic.

### Checkpoint 2: Assign Seeded Levels

Validate `HnswConfig`, then sample one capped geometric level per dataset row from the supplied deterministic generator.
Store the sampled levels so two builds with the same seed can be compared directly.

As rows arrive, extend every existing layer's adjacency storage and create missing upper layers through the row's sampled
level. Rows below a layer's membership threshold keep an empty adjacency list in that layer.

### Checkpoint 3: Build the Layered Graph

Implement the insertion descent and connection loops from the algorithm above. Reuse the Chapter 3 search and pruning
rules. At every connected layer, remove rejected edges from both endpoints and preserve the same deterministic neighbor
order.

Update the global entry point only when the new row's level is higher than the previous top level.

### Checkpoint 4: Search from Top to Bottom

Implement `search_with_ef`. Validate the query and search width, greedily descend every upper layer, then call
`search_layer` at layer zero with `ef_search.max(k)`. Truncate the nearest-first result to `k`.

Run the focused test:

```sh
cd rust
cargo test -p vector-core-starter --test indexes hnsw_is_seeded_and_high_ef_recovers_neighbors
```

The test checks seeded level assignment, nested membership, degree bounds, reciprocal edges, and exact overlap on one
connected fixture at a high search width. It does not make HNSW exact for arbitrary data or smaller budgets.

### Checkpoint 5: Use HNSW from SQL

Run the Chapter 4 SQLLogicTest:

```sh
cargo test -p vector-datafusion-starter --test sqllogictest day4_hnsw_sql
```

The unchanged SQL boundary now exposes the hierarchical index:

```text
SortExec: TopK(fetch=5), ...
  VectorIndexScanExec: index=hnsw, metric=Euclidean, query_dim=3, fetch=Some(5), ordered=false
```

HNSW selects candidates; DataFusion still owns final SQL ordering. Filters and incompatible distance expressions remain on
the exact scan.

## Review Your Chapter 4 Result

After the focused test and SQLLogicTest pass, choose one promoted row and one query and explain:

- why membership must be nested across layers;
- how one entry point moves from the top layer to layer zero;
- why upper layers use greedy search while layer zero keeps a beam;
- when the global entry point changes; and
- how the seed affects graph structure and a fair recall comparison.

Keep this chapter focused on an immutable in-memory HNSW index. Deletion, concurrent mutation, persistence, production
neighbor-diversification heuristics, and adaptive search budgets remain outside this checkpoint.

{{#include copyright.md}}
