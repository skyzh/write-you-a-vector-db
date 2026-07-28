use std::sync::Arc;

use datafusion::common::Result;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use vector_core::{HnswConfig, IndexConfig, Metric};
use vector_datafusion::{VectorRow, VectorTable};

const QUERY: &str = "SELECT id, payload FROM points \
    ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3";

fn table() -> Result<VectorTable> {
    VectorTable::try_new(
        vec![
            VectorRow::new(1, vec![1.0, 0.0, 0.0], "one"),
            VectorRow::new(2, vec![0.9, 0.1, 0.0], "two"),
            VectorRow::new(3, vec![0.0, 1.0, 0.0], "three"),
            VectorRow::new(4, vec![-1.0, 0.0, 0.0], "four"),
            VectorRow::new(5, vec![0.0, 0.0, 1.0], "five"),
        ],
        Metric::Cosine,
        IndexConfig::Hnsw(HnswConfig {
            max_connections: 3,
            ef_construction: 5,
            ef_search: 5,
            max_level: 4,
            seed: 7,
        }),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let baseline =
        SessionContext::new_with_config(SessionConfig::new().with_enable_sort_pushdown(false));
    baseline.register_table("points", Arc::new(table()?))?;
    println!("Exact DataFusion plan:");
    baseline
        .sql(&format!("EXPLAIN {QUERY}"))
        .await?
        .show()
        .await?;

    let indexed = SessionContext::new();
    indexed.register_table("points", Arc::new(table()?))?;
    println!("\nVector-index plan:");
    indexed
        .sql(&format!("EXPLAIN {QUERY}"))
        .await?
        .show()
        .await?;
    println!("\nResults:");
    indexed.sql(QUERY).await?.show().await?;
    Ok(())
}
