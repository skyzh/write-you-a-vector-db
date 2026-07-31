# Vector Core Starter

In the five Rust chapters, you will use this crate to:

1. validate the in-memory dataset used by the DataFusion table;
2. implement recall measurement and IVFFlat;
3. build and search a bounded-degree NSW graph;
4. add seeded hierarchy with HNSW; and
5. compare exact, IVFFlat, NSW, and HNSW search on one benchmark workload.

Optional Chapter 6 uses this crate to compress IVFFlat residuals with product
quantization and rerank a candidate shortlist with exact distances.

Start with the existing metric math, deterministic `FlatIndex`, and top-k
helpers. Run commands from the parent `rust/` workspace.
