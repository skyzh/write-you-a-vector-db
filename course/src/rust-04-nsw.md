# Navigate a Proximity Graph with NSW

> **Chapter 3**
>
> Complete [Restrict Search with IVFFlat](./rust-03-ivfflat.md) first. Finish with a bounded-degree NSW graph, best-first
> search measured against exact results, and the same SQL top-k running through `index=nsw`.

NSW is the graph-based building block of HNSW. It starts from one or more entry points and follows graph edges toward
vectors closer to the query. Because it explores only connected neighbors instead of comparing every stored vector, it
can answer with less work and can also stop before reaching the true nearest neighbor.

## Search One Layer

### One Entry Point and One Neighbor

The first diagram shows an NSW graph in two dimensions. The highlighted red vertex is the entry point, and the query
vector is elsewhere in the space. Search begins at the entry point because the graph has no global ordering or centroid
that points directly to the answer.

![One entry point begins the NSW walk](./vector-db/05-nsw-explore-1.svg)

Compare the query with every neighbor of the current vertex and move toward a closer neighbor. Repeating this greedy step
walks through the graph toward the query.

![A greedy step moves to a closer neighbor](./vector-db/05-nsw-explore-2.svg)

The walk stops when none of the current vertex's neighbors is closer. It may stop at a local minimum instead of the
globally nearest vector, which is why NSW is approximate.

### Multiple Entry Points and k Neighbors

Now ask for three neighbors while starting from two entry points. Maintain three pieces of state:

- `C`, a min-heap whose nearest candidate is the next vertex to explore;
- `W`, a max-heap whose top is the worst of the best visited vertices; and
- `visited`, a set that prevents a vertex from being expanded twice.

Seed all three structures from the entry points. The next diagram shows the state before any vertex is expanded.

![Seed the candidate and result frontiers](./vector-db/05-nsw-explore-3.svg)

Pop the nearest item from `C`. For each unseen neighbor, compute its distance once. If it can still improve the bounded
result frontier, add it to both `C` and `W`, then keep only the best three vertices in `W`.

![Expand the nearest candidate and update both frontiers](./vector-db/05-nsw-explore-4.svg)

A later candidate may have only neighbors that are already visited. Expanding it adds nothing, but the search can continue
with the remaining candidates.

![Visited neighbors are not expanded twice](./vector-db/05-nsw-explore-5.svg)

The second entry point reaches another part of the graph. It does not guarantee exact search, but it reduces the chance
that one poorly placed entry point traps the walk in the wrong region.

![A second entry point opens another region](./vector-db/05-nsw-explore-6.svg)

Continue popping the nearest candidate and updating `W`. Even after many vertices have been visited, `W` retains only the
best search-width candidates found so far.

![The result frontier keeps the nearest visited vertices](./vector-db/05-nsw-explore-7.svg)

Once `W` is full, compare the nearest pending candidate with its worst result. If the pending candidate is farther away,
no queued path can improve the current frontier, so the search stops.

![The nearest pending candidate is worse than the full result frontier](./vector-db/05-nsw-explore-8.svg)

```text
C = entry points as a min-heap by distance
W = unique entry points as a bounded max-heap by distance
visited = unique entry points

while C is not empty:
    candidate = C.pop_nearest()
    if W is full and candidate is worse than W.worst:
        break

    for neighbor in candidate.neighbors:
        if neighbor is already visited:
            continue
        mark neighbor visited
        if W is not full or neighbor is better than W.worst:
            C.push(neighbor)
            W.push(neighbor)
            trim W to the search width

return W from nearest to farthest
```

**Prediction:** If the graph has two disconnected components and every entry point is in the first component, can this
search return a vertex from the second? Explain why changing the heap width cannot create a missing edge.

## Insert into the Graph

Insert rows one at a time. To place a new vector, search the existing graph with width `ef_construction` and select its
nearest `max_connections` candidates.

![Choose the new vector's nearest graph neighbors](./vector-db/05-nsw-insert-1.svg)

Add reciprocal edges between the new vertex and each selected neighbor. Some endpoints may now exceed the degree cap.

![New reciprocal edges can exceed the degree cap](./vector-db/05-nsw-insert-2.svg)

For every overfull endpoint, sort its neighbors by distance from that endpoint and retain only the closest configured
number. Distance ties use row offset so equal inputs produce the same graph.

![Choose the connections that survive pruning](./vector-db/05-nsw-insert-3.svg)

Remove every rejected edge from both endpoints. The final graph has no self-edges or duplicate edges, every remaining
edge is reciprocal, and each vertex stays within the degree cap.

![Pruning leaves a bounded reciprocal graph](./vector-db/05-nsw-insert-4.svg)

## Build NSW in Rust

You will modify:

```text
rust/vector-starter/core/src/graph.rs
rust/vector-starter/core/src/nsw.rs
```

The starter already exposes `NswConfig`, `NswIndex`, the shared `Neighbor` ordering, and bounded `TopK` helpers. Keep the
public APIs, tests, metric behavior, and Chapter 1 DataFusion matcher unchanged.

### What Must Hold, and What Breaks If It Doesn't

Your graph budget must satisfy `max_connections > 0`,
`ef_construction >= max_connections`, and `ef_search > 0`. A zero
`max_connections` produces isolated nodes; an `ef_search` smaller than
`k` silently returns fewer results than requested.

One layer search must compute each visited row's query distance at most
once. Computing it again on revisit wastes work and can make pruning
decisions inconsistent if floating-point rounding differs across calls.

Two frontiers drive the search: `C` expands the nearest pending
candidate while bounded `W` tracks the worst retained result. Confusing
these roles can discard a candidate that would have led to a better
path.

Traversal stops only when `W` is full and the nearest pending candidate
is worse than `W`'s worst member. Stopping earlier can miss a closer
neighbor; stopping later cannot improve the result and only burns work.

Adjacency lists must contain no duplicates or self-edges. Every edge
must appear at both endpoints with degree at most `max_connections`. A
one-sided edge makes one node reachable but the other unreachable in
reverse; a duplicate changes the pruning budget.

All result and pruning ties use row offset after distance. Without a
deterministic tie-break, two runs with identical data and seed can
produce different result sets.

### Checkpoint 1: Search a Supplied Layer

Implement `search_layer` in `graph.rs`. Respect `allowed_rows` while building: when row `r` is inserted, only rows
`0..r` exist in the searchable graph. Ignore duplicate or out-of-range entry points, and return nearest-first results.

Use `ef.max(1)` as the frontier width. A larger `ef` explores and retains more candidates; it may improve recall but does
not make a disconnected graph connected.

### Checkpoint 2: Connect and Prune

Implement `prune_neighbors`. Remove duplicates, order neighbors by their distance from the owner, break ties by row
offset, and truncate to `max_connections`.

Then implement `NswIndex::try_new`. Validate the dataset and configuration, add the first row without searching, and
insert each later row through the existing graph. When pruning rejects an edge, remove it from both endpoints so I5 still
holds.

### Checkpoint 3: Query and Compare with Exact Search

Implement `search_with_ef`. Validate the query and search from the graph entry point with width `ef_search.max(k)`, then
return at most the nearest `k` results.

Run the focused graph test:

```sh
cd rust
cargo test -p vector-core-starter --test indexes nsw_high_ef_matches_exact_search_on_connected_fixture
```

The fixture checks a connected graph at a high search width, exact top-k overlap, reciprocal edges, and the degree cap.
It does not claim exact recall for every dataset or smaller search width.

### Checkpoint 4: Use NSW from SQL

Run the Chapter 3 SQLLogicTest:

```sh
cargo test -p vector-datafusion-starter --test sqllogictest day3_nsw_sql
```

The SQL text and optimizer rule remain unchanged. Only the selected core index changes:

```text
SortExec: TopK(fetch=5), ...
  VectorIndexScanExec: index=nsw, metric=Euclidean, query_dim=3, fetch=Some(5), ordered=false
```

The NSW graph chooses candidate row offsets. DataFusion's bounded sort still owns final SQL ordering, and unsupported query
shapes still use the exact scan.

## Review Your Chapter 3 Result

After the focused core test and SQLLogicTest pass, choose one insertion and one query and explain:

- why candidate and result frontiers need opposite heap orderings;
- which comparison permits early stopping;
- how a rejected edge is removed from both adjacency lists;
- how `ef_search` changes work and recall without changing SQL; and
- why a disconnected component remains unreachable without an entry point or edge into it.

Keep this chapter focused on one immutable graph layer. Hierarchy, deletion, concurrent mutation, persistence, and
sophisticated neighbor-diversification heuristics remain outside this checkpoint.

{{#include copyright.md}}
