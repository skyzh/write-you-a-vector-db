use datafusion::common::Result;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use vector_core::{HnswConfig, IndexConfig, IvfFlatConfig, Metric};
use vector_datafusion::{
    VectorIndexAttachment, VectorRow, vector_mem_table, with_vector_indexes,
    with_vector_search_options,
};

const QUERY: &str = "SELECT id, payload FROM points \
    ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3";

async fn context(config: IndexConfig) -> Result<SessionContext> {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(vec![
        VectorRow::new(1, vec![1.0, 0.0, 0.0], "one"),
        VectorRow::new(2, vec![0.9, 0.1, 0.0], "two"),
        VectorRow::new(3, vec![0.0, 1.0, 0.0], "three"),
        VectorRow::new(4, vec![-1.0, 0.0, 0.0], "four"),
        VectorRow::new(5, vec![0.0, 0.0, 1.0], "five"),
    ])?;
    base.register_table("points", table.clone())?;
    let attachment = VectorIndexAttachment::try_new(
        &base,
        "points",
        &table,
        "embedding",
        Metric::Cosine,
        config,
    )
    .await?;
    Ok(with_vector_indexes(&base, vec![attachment]))
}

#[tokio::main]
async fn main() -> Result<()> {
    let exact = context(IndexConfig::Flat).await?;
    println!("Vector-index plan and result:");
    exact.sql(&format!("EXPLAIN {QUERY}")).await?.show().await?;
    exact.sql(QUERY).await?.show().await?;

    let ivf = context(IndexConfig::IvfFlat(IvfFlatConfig {
        partitions: 2,
        probes: 2,
        iterations: 8,
        seed: 7,
    }))
    .await?;
    println!("\nIVFFlat-index plan and result:");
    ivf.sql(&format!("EXPLAIN {QUERY}")).await?.show().await?;
    ivf.sql(QUERY).await?.show().await?;

    let hnsw = context(IndexConfig::Hnsw(HnswConfig {
        max_connections: 3,
        ef_construction: 5,
        ef_search: 5,
        max_level: 4,
        seed: 7,
    }))
    .await?;
    println!("\nHNSW-index plan and result:");
    hnsw.sql(&format!("EXPLAIN {QUERY}")).await?.show().await?;
    hnsw.sql(QUERY).await?.show().await?;
    Ok(())
}
