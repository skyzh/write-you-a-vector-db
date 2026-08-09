use std::sync::Arc;

use datafusion::arrow::array::{
    ArrayRef, FixedSizeListArray, Float32Array, Int32Array, StringArray, UInt64Array,
};
use datafusion::arrow::buffer::NullBuffer;
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use vector_core::{HnswConfig, IndexConfig, IvfPqConfig, Metric};
use vector_datafusion::{VectorRow, VectorTable, with_vector_search_options};

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

fn vector_array<const N: usize>(vectors: &[[f32; N]]) -> ArrayRef {
    let item = Arc::new(Field::new("item", DataType::Float32, false));
    Arc::new(
        FixedSizeListArray::try_new(
            item,
            i32::try_from(N).unwrap(),
            Arc::new(Float32Array::from(
                vectors
                    .iter()
                    .flat_map(|vector| vector.iter().copied())
                    .collect::<Vec<_>>(),
            )),
            None,
        )
        .unwrap(),
    )
}

fn arbitrary_batches() -> Vec<RecordBatch> {
    let item = Arc::new(Field::new("item", DataType::Float32, false));
    let schema = Arc::new(Schema::new(vec![
        Field::new("label", DataType::Utf8, false),
        Field::new(
            "alternate_vector",
            DataType::FixedSizeList(Arc::clone(&item), 3),
            false,
        ),
        Field::new("score", DataType::Int32, false),
        Field::new(
            "features",
            DataType::FixedSizeList(Arc::clone(&item), 3),
            false,
        ),
        Field::new("external_id", DataType::UInt64, false),
    ]));
    vec![
        RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(StringArray::from(vec!["west", "north"])),
                vector_array(&[[1.0, 0.0, 0.0], [0.9, 0.1, 0.0]]),
                Arc::new(Int32Array::from(vec![40, 30])),
                vector_array(&[[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
                Arc::new(UInt64Array::from(vec![104, 103])),
            ],
        )
        .unwrap(),
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["east-2", "east-1"])),
                vector_array(&[[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]),
                Arc::new(Int32Array::from(vec![20, 10])),
                vector_array(&[[0.9, 0.1, 0.0], [1.0, 0.0, 0.0]]),
                Arc::new(UInt64Array::from(vec![102, 101])),
            ],
        )
        .unwrap(),
    ]
}

fn arbitrary_context() -> SessionContext {
    let context = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = VectorTable::try_new_batches(
        arbitrary_batches(),
        "features",
        Metric::Cosine,
        IndexConfig::Flat,
    )
    .unwrap();
    assert_eq!(table.vector_column(), "features");
    context.register_table("items", Arc::new(table)).unwrap();
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
async fn arbitrary_schema_and_multi_batch_row_ids_preserve_complete_rows() {
    let context = arbitrary_context();
    let sql = "SELECT * FROM items \
               ORDER BY cosine_distance(features, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");

    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let batch = &batches[0];
    assert_eq!(
        batch
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().as_str())
            .collect::<Vec<_>>(),
        [
            "label",
            "alternate_vector",
            "score",
            "features",
            "external_id"
        ]
    );
    let labels = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let ids = batch
        .column(4)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(
        labels.iter().collect::<Vec<_>>(),
        [Some("east-1"), Some("east-2")]
    );
    assert_eq!(ids.values(), &[101, 102]);
}

#[tokio::test]
async fn ivf_pq_keeps_original_vectors_and_row_ids_for_arbitrary_rows() {
    let context = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = VectorTable::try_new_batches(
        arbitrary_batches(),
        "features",
        Metric::Euclidean,
        IndexConfig::IvfPq(IvfPqConfig {
            partitions: 2,
            probes: 2,
            iterations: 4,
            subquantizers: 1,
            codebook_size: 2,
            rerank: 4,
            seed: 7,
        }),
    )
    .unwrap();
    context.register_table("items", Arc::new(table)).unwrap();

    let sql = "SELECT * FROM items \
               ORDER BY array_distance(features, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("index=ivf_pq"), "{plan}");
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(4)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[101, 102]);
}

#[tokio::test]
async fn projection_and_filter_keep_all_non_index_columns_correct() {
    let context = arbitrary_context();
    let count = context
        .sql("SELECT COUNT(*) FROM items")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let count = count[0]
        .column(0)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap();
    assert_eq!(count.values(), &[4]);

    let projected_sql = "SELECT label, score FROM items \
                         ORDER BY cosine_distance(features, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, projected_sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    let projected = context
        .sql(projected_sql)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let labels = projected[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let scores = projected[0]
        .column(1)
        .as_any()
        .downcast_ref::<Int32Array>()
        .unwrap();
    assert_eq!(scores.values(), &[10, 20]);
    assert_eq!(
        labels.iter().collect::<Vec<_>>(),
        [Some("east-1"), Some("east-2")]
    );

    let reordered = context
        .sql(
            "SELECT score, label FROM items \
             ORDER BY cosine_distance(features, [1.0, 0.0, 0.0]) LIMIT 1",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(reordered[0].schema().field(0).name(), "score");
    assert_eq!(reordered[0].schema().field(1).name(), "label");

    let filtered_sql = "SELECT external_id, label FROM items WHERE score >= 20 \
                        ORDER BY cosine_distance(features, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, filtered_sql).await;
    assert!(plan.contains("VectorScanExec"), "{plan}");
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
    let filtered = context
        .sql(filtered_sql)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let ids = filtered[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[102, 103]);
}

#[tokio::test]
async fn only_the_selected_vector_column_and_top_k_match_the_index() {
    let context = arbitrary_context();
    let wrong_column = "SELECT external_id FROM items \
                        ORDER BY cosine_distance(alternate_vector, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, wrong_column).await;
    assert!(plan.contains("VectorScanExec"), "{plan}");
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
    let batches = context
        .sql(wrong_column)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[104, 103]);

    let no_limit = "SELECT external_id FROM items \
                    ORDER BY cosine_distance(features, [1.0, 0.0, 0.0])";
    let plan = explain(&context, no_limit).await;
    assert!(plan.contains("VectorScanExec"), "{plan}");
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
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

#[test]
fn arbitrary_schema_rejects_missing_or_wrong_type_before_index_build() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("name", DataType::Utf8, false),
        Field::new("score", DataType::Int32, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["one", "two"])),
            Arc::new(Int32Array::from(vec![1, 2])),
        ],
    )
    .unwrap();
    let invalid_index = IndexConfig::IvfPq(IvfPqConfig {
        partitions: 0,
        probes: 0,
        iterations: 0,
        subquantizers: 0,
        codebook_size: 0,
        rerank: 0,
        seed: 0,
    });

    let missing = VectorTable::try_new_batch(
        batch.clone(),
        "missing",
        Metric::Euclidean,
        invalid_index.clone(),
    )
    .unwrap_err();
    assert!(
        missing
            .to_string()
            .contains("does not exist or is ambiguous")
    );

    let wrong_type =
        VectorTable::try_new_batch(batch, "score", Metric::Euclidean, invalid_index).unwrap_err();
    assert!(
        wrong_type
            .to_string()
            .contains("must be FixedSizeList<Float32>")
    );
}

#[test]
fn arbitrary_schema_does_not_use_a_user_id_column_as_row_identity() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new(
            "features",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 2),
            false,
        ),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt64Array::from(vec![7, 7])),
            vector_array(&[[1.0, 0.0], [0.0, 1.0]]),
        ],
    )
    .unwrap();

    let table = VectorTable::try_new_batch(batch, "features", Metric::Euclidean, IndexConfig::Flat)
        .unwrap();
    assert_eq!(table.vector_column(), "features");
}

#[test]
fn arbitrary_schema_rejects_dimension_drift_and_null_vectors() {
    let schema_2d = Arc::new(Schema::new(vec![Field::new(
        "features",
        DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 2),
        true,
    )]));
    let schema_3d = Arc::new(Schema::new(vec![Field::new(
        "features",
        DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 3),
        false,
    )]));
    let two_dimensions =
        RecordBatch::try_new(Arc::clone(&schema_2d), vec![vector_array(&[[1.0, 0.0]])]).unwrap();
    let three_dimensions =
        RecordBatch::try_new(schema_3d, vec![vector_array(&[[1.0, 0.0, 0.0]])]).unwrap();
    let drift = VectorTable::try_new_batches(
        vec![two_dimensions, three_dimensions],
        "features",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .unwrap_err();
    assert!(drift.to_string().contains("does not match"));

    let null_vectors = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, false)),
        2,
        Arc::new(Float32Array::from(vec![1.0, 0.0, 0.0, 1.0])),
        Some(NullBuffer::from(vec![true, false])),
    )
    .unwrap();
    let null_batch =
        RecordBatch::try_new(schema_2d, vec![Arc::new(null_vectors) as ArrayRef]).unwrap();
    let null =
        VectorTable::try_new_batch(null_batch, "features", Metric::Euclidean, IndexConfig::Flat)
            .unwrap_err();
    assert!(null.to_string().contains("contains null at row 1"));
}
