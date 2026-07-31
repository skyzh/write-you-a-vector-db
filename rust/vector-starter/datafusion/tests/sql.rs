use std::sync::Arc;

use datafusion::arrow::array::UInt64Array;
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use vector_core::{HnswConfig, IndexConfig, IvfPqConfig, Metric};
use vector_datafusion_starter::{VectorRow, VectorTable, with_vector_search_options};

fn rows() -> Vec<VectorRow> {
    vec![
        VectorRow::new(10, vec![1.0, 0.0, 0.0], "east"),
        VectorRow::new(20, vec![0.9, 0.1, 0.0], "east"),
        VectorRow::new(30, vec![0.0, 1.0, 0.0], "north"),
        VectorRow::new(40, vec![-1.0, 0.0, 0.0], "west"),
        VectorRow::new(50, vec![0.0, 0.0, 1.0], "up"),
    ]
}

fn context(metric: Metric, config: IndexConfig) -> SessionContext {
    let context = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = VectorTable::try_new(rows(), metric, config).unwrap();
    context.register_table("points", Arc::new(table)).unwrap();
    context
}

async fn explain(context: &SessionContext, sql: &str) -> String {
    let batches = context
        .sql(&format!("EXPLAIN {sql}"))
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    pretty_format_batches(&batches).unwrap().to_string()
}

#[tokio::test]
async fn compatible_top_k_uses_vector_index_scan_and_keeps_sort() {
    let context = context(Metric::Cosine, IndexConfig::Flat);
    let sql = "SELECT id, payload FROM points \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(plan.contains("ordered=false"), "{plan}");
    assert!(plan.contains("SortExec: TopK(fetch=2)"), "{plan}");
    assert!(plan.contains("fetch=Some(2)"), "{plan}");

    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[10, 20]);
}

#[tokio::test]
async fn ordered_session_mode_allows_sort_elision() {
    let context = context(Metric::Cosine, IndexConfig::Flat);
    context
        .sql("SET vector_search.ordered = true")
        .await
        .unwrap();
    let sql = "SELECT id, payload FROM points \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(plan.contains("ordered=true"), "{plan}");
    assert!(!plan.contains("SortExec"), "{plan}");
}

#[tokio::test]
async fn filter_keeps_datafusion_exact_fallback() {
    let context = context(Metric::Cosine, IndexConfig::Flat);
    let sql = "SELECT id FROM points WHERE payload = 'north' \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("SortExec"), "{plan}");
    assert!(plan.contains("VectorScanExec"), "{plan}");
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
}

#[tokio::test]
async fn unsafe_sort_shapes_are_not_lowered() {
    let context = context(Metric::Cosine, IndexConfig::Flat);
    for sql in [
        "SELECT id FROM points ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) DESC LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]), id LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, embedding) LIMIT 2",
    ] {
        let plan = explain(&context, sql).await;
        assert!(plan.contains("SortExec"), "{plan}");
        assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
    }
}

#[tokio::test]
async fn hnsw_is_visible_in_explain() {
    let context = context(
        Metric::Cosine,
        IndexConfig::Hnsw(HnswConfig {
            max_connections: 3,
            ef_construction: 5,
            ef_search: 5,
            max_level: 4,
            seed: 7,
        }),
    );
    let plan = explain(
        &context,
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2",
    )
    .await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(plan.contains("index=hnsw"), "{plan}");
    assert!(plan.contains("fetch=Some(2)"), "{plan}");
}

#[tokio::test]
async fn ivf_pq_is_visible_in_explain() {
    let context = context(
        Metric::Euclidean,
        IndexConfig::IvfPq(IvfPqConfig {
            partitions: 2,
            probes: 2,
            iterations: 4,
            subquantizers: 1,
            codebook_size: 4,
            rerank: 5,
            seed: 7,
        }),
    );
    let plan = explain(
        &context,
        "SELECT id FROM points ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2",
    )
    .await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(plan.contains("index=ivf_pq"), "{plan}");
    assert!(plan.contains("fetch=Some(2)"), "{plan}");
}

#[tokio::test]
async fn dot_product_requires_descending_order() {
    let context = context(Metric::Dot, IndexConfig::Flat);
    let sql = "SELECT id FROM points \
               ORDER BY inner_product(embedding, [1.0, 0.0, 0.0]) DESC LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[10, 20]);
}

#[test]
fn table_rejects_duplicate_ids() {
    let error = VectorTable::try_new(
        vec![
            VectorRow::new(1, vec![1.0, 0.0], "left"),
            VectorRow::new(1, vec![0.0, 1.0], "right"),
        ],
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .unwrap_err();
    assert!(error.to_string().contains("duplicate vector id 1"));
}
