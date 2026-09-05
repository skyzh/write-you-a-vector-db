# Vector DataFusion Starter

On Day 1, you will use this crate to build the introductory Arrow
`MemTable`, attach an index to one selected vector field in an arbitrary schema,
and implement a conservative physical-plan rewrite. Its SQLLogicTests establish
the optimizer boundary that IVFFlat, NSW, HNSW, and IVF-PQ reuse through
Day 5.

The crate includes the execution helpers you need, so you can focus on the
explicit Day 1 TODOs in the guide.

From the repository root, finish with `cargo x test-day 1`, then
`cargo x test-through 1`.
