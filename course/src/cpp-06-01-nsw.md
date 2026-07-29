# NSW (Navigable Small Worlds) Index

{{#include cpp-deprecation.md}}

NSW is the graph-based building block of HNSW: it starts from one or more entry points and greedily follows neighbors
closer to the query vector. This chapter implements it as the last fully specified checkpoint in the C++ course. The
starter represents the graph as `layers_[0]` inside `HNSWIndex` so the next, optional chapter can add hierarchy.

Complete the previous chapters first. You will likely modify:

```text
src/include/storage/index/hnsw_index.h
src/storage/index/hnsw_index.cpp
```

*Related readings:*

- [Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs](https://arxiv.org/abs/1603.09320)
- [HNSW in Pinecone's Faiss guide](https://www.pinecone.io/learn/series/faiss/hnsw/)

## Search One Layer

### One Entry Point and One Neighbor

The first diagram shows an NSW graph in two dimensions. The highlighted red vertex is the entry point, and the query
vector is elsewhere in the space. Search begins at the entry point because the graph does not provide a global ordering
or a centroid that points directly to the answer.

![One entry point](./vector-db/05-nsw-explore-1.svg)

Compare the query with every neighbor of the current vertex and move to a neighbor that is closer. Repeating this greedy
step walks through the graph toward the query.

![Greedy movement](./vector-db/05-nsw-explore-2.svg)

The walk stops when none of the current vertex's neighbors is closer. Because it explores only graph edges, it can stop at
a local minimum instead of the globally nearest vector; NSW is therefore an approximate index.

### Multiple Entry Points and k Neighbors

Now ask for three neighbors while starting from two entry points. Searching multiple regions reduces the chance that one
poorly placed entry point traps the walk in the wrong part of the graph. For k-nearest-neighbor search, maintain:

- `C`, a min-heap of candidates to explore, with the nearest candidate on top;
- `W`, a max-heap of the best visited candidates, with the worst retained result on top; and
- `visited`, a set that prevents repeated graph expansion.

Seed all three structures from both entry points. The next diagram shows the initial state before any vertex is expanded.

![Seed the queues](./vector-db/05-nsw-explore-3.svg)

The nearest item in `C` is entry point 1. Pop it, mark its unseen neighbors as visited, add promising neighbors to `C` and
`W`, and keep only the three best visited candidates in `W`.

![Explore neighbors](./vector-db/05-nsw-explore-4.svg)

The next candidate is the highlighted vertex below. Its neighbors have already been visited, so expanding it does not add
anything to either queue.

![Skip visited neighbors](./vector-db/05-nsw-explore-5.svg)

Entry point 2 is now the nearest unexplored candidate. Expanding it adds candidates from a different part of the graph,
which is the benefit of seeding the search from more than one location.

![Explore the second entry point](./vector-db/05-nsw-explore-6.svg)

Continue popping the nearest candidate and updating `W`. Even when many vertices have been visited, `W` retains only the
three closest ones found so far.

![Continue the search](./vector-db/05-nsw-explore-7.svg)

Eventually the nearest candidate left in `C` is farther from the query than the worst result in the full `W`. No queued
candidate can improve the result, so the search stops.

![Stop condition](./vector-db/05-nsw-explore-8.svg)

```text
C = entry_points as a min-heap by distance
W = unique entry_points as a max-heap by distance
visited = unique entry_points

while C is not empty:
    candidate = C.pop_nearest()
    if W is full and distance(candidate) > distance(W.worst):
        break

    for neighbor in candidate.neighbors:
        if neighbor is already visited:
            continue
        mark neighbor visited
        if W is not full or distance(neighbor) < distance(W.worst):
            C.push(neighbor)
            W.push(neighbor)
            trim W to the search width

return W sorted from nearest to farthest
```

### Why Multiple Entry Points Matter

If the same graph asks for two neighbors but starts only from entry point 1, the search can reach the state below and
stop: every remaining candidate in `C` is worse than both results in `W`, even though the graph contains closer vertices
in another region. A second entry point seeds that region directly; it does not guarantee exact search, but it reduces
this failure mode.

![One entry point stops early](./vector-db/05-nsw-explore-5-stop.svg)

**Course rules for `NSW::SearchLayer`:**

- return an empty vector for `limit = 0`, no entry points, or an empty layer;
- ignore duplicate entry-point IDs and never visit a vertex more than once;
- use `dist_fn_` for every comparison; and
- return at most `limit` vertex IDs, sorted from nearest to farthest.

## Insert into the Graph

To insert the highlighted new vector, search the existing graph with width `ef_construction`, then select its nearest `m`
candidates as neighbors. The first diagram shows those selected connections.

![Choose neighbors](./vector-db/05-nsw-insert-1.svg)

`NSW::Connect` creates an undirected edge between the new vertex and each selected neighbor. Those new edges can put an
existing vertex over the layer's `m_max_` degree cap, as in the second diagram.

![Too many edges](./vector-db/05-nsw-insert-2.svg)

For every overfull vertex, re-select its nearest `m_max_` neighbors. The third diagram marks the edges that survive this
pruning decision.

![Select retained edges](./vector-db/05-nsw-insert-3.svg)

Finally, remove every rejected edge from both endpoints. The last diagram is the resulting graph; updating only the
overfull vertex would leave one-sided edges and break the undirected-graph invariant.

![Pruned graph](./vector-db/05-nsw-insert-4.svg)

**Course rules for insertion:**

- The first vertex is added without searching or connecting.
- Do not create self-edges or duplicate edges.
- Keep `edges_[a]` and `edges_[b]` symmetric after both connection and pruning.
- `SelectNeighbors` returns at most `m` unique IDs ordered by `dist_fn_`.
- Add the new vertex to the layer exactly once.

The starter parameters mean:

- `m_`: how many neighbors a new vertex selects;
- `ef_construction_`: the insertion-search width;
- `ef_search_`: the query-search width;
- `m_max_`: the upper-layer degree cap reserved for HNSW; and
- `m_max_0_`: the layer-0 degree cap. The starter derives it as `m_ * m_` and assigns it to `layers_[0].m_max_`.

Require `m > 1`, `ef_construction >= m`, and `ef_search >= 1`. The first condition also keeps the starter's
`m_l_ = 1 / log(m)` finite for the optional HNSW extension.

For a SQL `LIMIT k`, search layer 0 with width `max(k, ef_search_)`, then select and return the nearest `k` RIDs. This
ensures that a request for more than `ef_search_` rows can still return `k` rows, while a larger `ef_search_` can improve
recall.

## Verify the Checkpoint

From `bustub-vectordb/build`, run:

```shell
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/vector.05-hnsw.slt --verbose
```

Confirm that results are sorted by distance, inserts after index construction are searchable, and the `LIMIT 5` query can
return five rows even though the test index uses `ef_search = 3`. Random build order can change tie ordering.

<details>

<summary>One-Layer NSW Reference</summary>

```text
{{#include vector.05-hnsw.slt.1.ref}}
```

</details>

Also compare an NSW query with `SET vector_index_method=none`. Exact Top-N is the oracle for recall, not a requirement that
every approximate result match.

**Prediction:** If the graph has two disconnected components and every entry point is in the first component, can
`SearchLayer` return a vertex from the second? Explain why the `visited` and stop-condition code cannot repair missing
connectivity.

You are done when you can trace one vertex ID through `vertices_`, `layers_[0].edges_`, `rids_`, and the table lookup, and
explain what would break if pruning removed only one side of an undirected edge.

## Optional Extensions

- Implement the paper's heuristic neighbor-selection rule.
- Add deletion and update support.
- Persist the graph after defining a stable on-disk layout.

{{#include copyright.md}}
