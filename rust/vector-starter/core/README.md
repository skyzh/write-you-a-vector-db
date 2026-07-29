# Vector Core Starter

This crate contains the learner-owned core pieces for the two Rust chapters:

1. validate the in-memory dataset used by the DataFusion table;
2. implement recall measurement and IVFFlat.
Metric math, deterministic exact top-k, and the flat oracle are supplied
because DataFusion already provides exact SQL expression, sort, and limit
execution. Run commands from the parent `rust/` workspace.
