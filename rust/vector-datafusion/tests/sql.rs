use std::sync::Arc;

use datafusion::arrow::array::UInt64Array;
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::execution::context::SessionContext;
use vector_core::{IndexConfig, Metric};
use vector_datafusion::{VectorRow, VectorTable};

fn rows() -> Vec<VectorRow> {
    vec![
        VectorRow::new(10, vec![1.0, 0.0, 0.0], "east"),
        VectorRow::new(20, vec![0.9, 0.1, 0.0], "east"),
        VectorRow::new(30, vec![0.0, 1.0, 0.0], "north"),
        VectorRow::new(40, vec![-1.0, 0.0, 0.0], "west"),
        VectorRow::new(50, vec![0.0, 0.0, 1.0], "up"),
    ]
}

async fn context(metric: Metric, config: IndexConfig) -> SessionContext {
    let context = SessionContext::new();
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
async fn compatible_top_k_uses_vector_index_scan() {
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
    let sql = "SELECT id, payload FROM points \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(!plan.contains("SortExec"), "{plan}");

    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[10, 20]);
}

#[tokio::test]
async fn filter_keeps_datafusion_exact_fallback() {
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
    let sql = "SELECT id FROM points WHERE payload = 'north' \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("SortExec"), "{plan}");
    assert!(plan.contains("VectorScanExec"), "{plan}");
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");

    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[30]);
}

#[tokio::test]
async fn wrong_metric_and_direction_are_not_lowered() {
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
    let wrong_metric = explain(
        &context,
        "SELECT id FROM points ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2",
    )
    .await;
    assert!(wrong_metric.contains("SortExec"), "{wrong_metric}");
    assert!(
        !wrong_metric.contains("VectorIndexScanExec"),
        "{wrong_metric}"
    );

    let wrong_direction = explain(
        &context,
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) DESC LIMIT 2",
    )
    .await;
    assert!(wrong_direction.contains("SortExec"), "{wrong_direction}");
    assert!(
        !wrong_direction.contains("VectorIndexScanExec"),
        "{wrong_direction}"
    );

    let multiple_keys = explain(
        &context,
        "SELECT id FROM points \
         ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]), id LIMIT 2",
    )
    .await;
    assert!(multiple_keys.contains("SortExec"), "{multiple_keys}");
    assert!(
        !multiple_keys.contains("VectorIndexScanExec"),
        "{multiple_keys}"
    );

    let non_literal = explain(
        &context,
        "SELECT id FROM points ORDER BY cosine_distance(embedding, embedding) LIMIT 2",
    )
    .await;
    assert!(non_literal.contains("SortExec"), "{non_literal}");
    assert!(
        !non_literal.contains("VectorIndexScanExec"),
        "{non_literal}"
    );
}

#[tokio::test]
async fn dot_product_requires_descending_order() {
    let context = context(Metric::Dot, IndexConfig::Flat).await;
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
