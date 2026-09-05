# Vector Core Starter

In the six Rust chapters, you will use this crate to:

1. validate the in-memory dataset used by the DataFusion table;
2. implement recall measurement and IVFFlat;
3. build and search a bounded-degree NSW graph;
4. add seeded hierarchy with HNSW;
5. compress IVFFlat residuals with product quantization and rerank a candidate
   shortlist with exact distances; and
6. compare Flat, IVFFlat, NSW, HNSW, and IVF-PQ on one Euclidean workload.

Start with the existing metric math, deterministic `FlatIndex`, and top-k
helpers. Run commands from the repository-root Cargo workspace.

At the end of Day `N`, run `cargo x test-day N` for the new public contract and
`cargo x test-through N` for every learner day completed so far.
