# HNSW (Hierarchical Navigable Small Worlds) Index (WIP)

{{#include cpp-deprecation.md}}

<div class="warning">

**Optional design sketch:** The pinned starter has fields for multiple NSW layers, but this page is not a complete learner
checkpoint. Its SQLLogicTest reuses the NSW queries and cannot prove that hierarchy was built or used. The previous NSW
chapter is the last fully specified and testable C++ assignment.

</div>

HNSW stacks multiple NSW layers to make graph search more efficient, much like a skip list or mipmap. Sparse upper layers
make long jumps; layer 0 still contains every vertex and produces the final candidates.

Files an extension would modify:

```text
src/include/storage/index/hnsw_index.h
src/storage/index/hnsw_index.cpp
```

*Related readings:*

- [Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs](https://arxiv.org/abs/1603.09320)
- [HNSW in Pinecone's Faiss guide](https://www.pinecone.io/learn/series/faiss/hnsw/)

## Layer Invariants

![HNSW architecture](./vector-db/06-hnsw-architecture.svg)

A complete extension should preserve these rules:

- layer 0 contains every vertex;
- membership is nested: a vertex in layer `L` also appears in every lower layer;
- each layer stores global vertex IDs into `vertices_` and `rids_`;
- upper layers use `m_max_`, while layer 0 uses `m_max_0_`;
- edges remain symmetric within each layer; and
- the top entry point belongs to the current highest nonempty layer.

The pinned header has no dedicated top-entry-point field. You may add one, or derive it consistently from the highest
layer. That representation is your choice; the invariants are not.

## Lookup

![HNSW lookup](./vector-db/06-hnsw-explore.svg)

Starting at the highest layer, use a width-1 greedy search to find the entry point for the next layer. At layer 0, use
`max(k, ef_search_)` candidates and return the nearest `k` RIDs in distance order.

```text
entry_points = [top_entry_point]
for level from highest_level down to 1:
    entry_points = layers[level].search(target, limit=1, entry_points)

candidates = layers[0].search(
    target,
    limit=max(k, ef_search),
    entry_points=entry_points,
)
return nearest k candidates
```

Return an empty result for an empty index or `k = 0`; do not call `DefaultEntryPoint()` on an empty layer.

## Insertion

Draw a random `U` strictly greater than zero and compute
\\( \text{level} = \lfloor -\ln(U) \times m_L \rfloor \\). The starter sets
\\( m_L = 1 / \ln(m) \\), which requires `m > 1`.

![Choose an insertion level](./vector-db/06-hnsw-insert-1.svg)

Search width 1 above the new vertex's target level. At the levels where the new vertex belongs, search
`ef_construction_` candidates, select the nearest `m_`, connect them, and prune both sides of rejected edges.

![Connect at each included layer](./vector-db/06-hnsw-insert-2.svg)

```text
if the index is empty:
    create layers 0 through target_level
    add the first vertex to every layer
    make it the top entry point
    return

entry_points = [top_entry_point]
for level from current_highest down to target_level + 1:
    entry_points = layers[level].search(target, limit=1, entry_points)

for level from min(current_highest, target_level) down to 0:
    candidates = layers[level].search(target, ef_construction, entry_points)
    neighbors = nearest m candidates
    add the new vertex to this layer and connect it to neighbors
    prune overfull endpoints while preserving symmetric edges
    entry_points = candidates

if target_level is above current_highest:
    create each missing upper layer with only the new vertex
    make the new vertex the top entry point
```

This outline still leaves engineering choices such as random seeding and helper layout to the implementer. It does not
replace the paper's full algorithm or define a released course checkpoint.

## Available Regression Check

From `bustub-vectordb/build`, the existing SQL regression is:

```shell
make -j8 sqllogictest
./bin/bustub-sqllogictest ../test/sql/vector.05-hnsw.slt --verbose
```

<details>

<summary>Multi-Layer Reference Output</summary>

```text
{{#include vector.05-hnsw.slt.2.ref}}
```

</details>

Passing this file proves only that the public SQL behavior still works. A real HNSW test must also inspect internal
structure: create enough vertices with a deterministic generator, assert that more than one layer exists, verify nested
membership and degree caps, and demonstrate that lookup visits an upper layer before layer 0.

Until those checks and a completed learner write-up exist, treat HNSW as an optional experiment rather than a completed
course outcome.

{{#include copyright.md}}
