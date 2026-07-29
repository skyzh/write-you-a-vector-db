use std::sync::Arc;

use datafusion::common::Result;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use vector_core::{IndexConfig, Metric};
use vector_datafusion::{VectorRow, VectorTable, with_vector_search_options};

const QUERY: &str = "SELECT id, payload FROM points \
    ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3";

fn table(config: IndexConfig) -> Result<VectorTable> {
    VectorTable::try_new(
        vec![
            VectorRow::new(1, vec![1.0, 0.0, 0.0], "one"),
            VectorRow::new(2, vec![0.9, 0.1, 0.0], "two"),
            VectorRow::new(3, vec![0.0, 1.0, 0.0], "three"),
            VectorRow::new(4, vec![-1.0, 0.0, 0.0], "four"),
            VectorRow::new(5, vec![0.0, 0.0, 1.0], "five"),
        ],
        Metric::Cosine,
        config,
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let exact = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    exact.register_table("points", Arc::new(table(IndexConfig::Flat)?))?;
    println!("Vector-index plan and result:");
    exact.sql(&format!("EXPLAIN {QUERY}")).await?.show().await?;
    exact.sql(QUERY).await?.show().await?;
    Ok(())
}
