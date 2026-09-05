use std::sync::Arc;

use datafusion::arrow::array::{
    ArrayRef, BooleanArray, FixedSizeListArray, Float32Array, Float64Array, Int32Array,
    StringArray, UInt32Array, UInt64Array,
};
use datafusion::arrow::buffer::NullBuffer;
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::catalog::MemorySchemaProvider;
use datafusion::common::Result as DataFusionResult;
use datafusion::datasource::MemTable;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use vector_core::{HnswConfig, IndexConfig, IvfPqConfig, Metric};
use vector_datafusion::{
    VectorIndexAttachment, VectorRow, vector_mem_table, with_vector_indexes,
    with_vector_search_options,
};

fn rows() -> Vec<VectorRow> {
    vec![
        VectorRow::new(10, vec![1.0, 0.0, 0.0], "east"),
        VectorRow::new(20, vec![0.9, 0.1, 0.0], "east"),
        VectorRow::new(30, vec![0.0, 1.0, 0.0], "north"),
        VectorRow::new(40, vec![-1.0, 0.0, 0.0], "west"),
        VectorRow::new(50, vec![0.0, 0.0, 1.0], "up"),
    ]
}

async fn attach(
    context: &SessionContext,
    table_name: &str,
    table: &Arc<MemTable>,
    vector_column: &str,
    metric: Metric,
    config: IndexConfig,
) -> SessionContext {
    let attachment =
        VectorIndexAttachment::try_new(context, table_name, table, vector_column, metric, config)
            .await
            .unwrap();
    with_vector_indexes(context, vec![attachment])
}

async fn context_with_batches(
    table_name: &str,
    batches: Vec<RecordBatch>,
    vector_column: &str,
    metric: Metric,
    config: IndexConfig,
) -> SessionContext {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = Arc::new(MemTable::try_new(batches[0].schema(), vec![batches]).unwrap());
    base.register_table(table_name, table.clone()).unwrap();
    attach(&base, table_name, &table, vector_column, metric, config).await
}

async fn attachment_result(
    batches: Vec<RecordBatch>,
    vector_column: &str,
    metric: Metric,
    config: IndexConfig,
) -> DataFusionResult<VectorIndexAttachment> {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = Arc::new(MemTable::try_new(batches[0].schema(), vec![batches])?);
    base.register_table("items", table.clone())?;
    VectorIndexAttachment::try_new(&base, "items", &table, vector_column, metric, config).await
}

async fn context(metric: Metric, config: IndexConfig) -> SessionContext {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(rows()).unwrap();
    base.register_table("points", table.clone()).unwrap();
    attach(&base, "points", &table, "embedding", metric, config).await
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

fn rich_schema_batches() -> Vec<RecordBatch> {
    let item = Arc::new(Field::new("item", DataType::Float32, false));
    let schema = Arc::new(Schema::new(vec![
        Field::new("doc_key", DataType::Utf8, false),
        Field::new("tenant_id", DataType::UInt32, false),
        Field::new("price", DataType::Float64, false),
        Field::new("inventory", DataType::Int32, false),
        Field::new(
            "text_embedding",
            DataType::FixedSizeList(Arc::clone(&item), 3),
            false,
        ),
        Field::new("image_embedding", DataType::FixedSizeList(item, 3), false),
        Field::new("active", DataType::Boolean, false),
    ]));
    vec![
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["alpha", "beta", "gamma", "delta"])),
                Arc::new(UInt32Array::from(vec![7, 8, 9, 10])),
                Arc::new(Float64Array::from(vec![10.5, 20.25, 30.75, 40.0])),
                Arc::new(Int32Array::from(vec![4, 3, 2, 1])),
                vector_array(&[
                    [1.0, 0.0, 0.0],
                    [0.9, 0.1, 0.0],
                    [0.0, 1.0, 0.0],
                    [-1.0, 0.0, 0.0],
                ]),
                vector_array(&[
                    [-1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.9, 0.1, 0.0],
                    [1.0, 0.0, 0.0],
                ]),
                Arc::new(BooleanArray::from(vec![true, false, true, false])),
            ],
        )
        .unwrap(),
    ]
}

async fn rich_schema_context() -> SessionContext {
    context_with_batches(
        "documents",
        rich_schema_batches(),
        "text_embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
}

async fn arbitrary_context() -> SessionContext {
    context_with_batches(
        "items",
        arbitrary_batches(),
        "features",
        Metric::Cosine,
        IndexConfig::Flat,
    )
    .await
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

fn assert_memtable_fallback(plan: &str) {
    assert!(plan.contains("DataSourceExec"), "{plan}");
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(!plan.contains("VectorOrderingExec"), "{plan}");
}

#[tokio::test]
async fn compatible_top_k_uses_vector_index_scan_and_keeps_sort() {
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
    let sql = "SELECT id, payload FROM points \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    assert!(plan.contains("ordered=false"), "{plan}");
    assert!(plan.contains("SortExec: TopK(fetch=2)"), "{plan}");
    assert!(plan.contains("fetch=Some(2)"), "{plan}");
    assert_eq!(plan.matches("VectorIndexScanExec").count(), 1, "{plan}");
    assert!(!plan.contains("DataSourceExec"), "{plan}");
    assert!(!plan.contains("VectorOrderingExec"), "{plan}");

    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[10, 20]);
}

#[tokio::test]
async fn ordinary_memtable_without_attachment_stays_exact() {
    let context = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(rows()).unwrap();
    context.register_table("points", table).unwrap();

    let sql = "SELECT id FROM points \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("SortExec"), "{plan}");
    assert_memtable_fallback(&plan);
}

#[tokio::test]
async fn attachment_cannot_cross_bind_another_memtable() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let indexed = vector_mem_table(rows()).unwrap();
    let other = vector_mem_table(vec![
        VectorRow::new(900, vec![9.0, 0.0, 0.0], "other-nine"),
        VectorRow::new(901, vec![8.0, 0.0, 0.0], "other-eight"),
    ])
    .unwrap();
    base.register_table("points", indexed.clone()).unwrap();
    base.register_table("other", other).unwrap();
    let context = attach(
        &base,
        "points",
        &indexed,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    let sql = "SELECT id FROM other \
               ORDER BY array_distance(embedding, [0.0, 0.0, 0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert_memtable_fallback(&plan);
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[901]);
}

#[tokio::test]
async fn shared_batch_alias_between_two_memtables_fails_closed() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let indexed = vector_mem_table(rows()).unwrap();
    let shared_batch = indexed.batches[0].read().await[0].clone();
    let alias = Arc::new(
        MemTable::try_new(shared_batch.schema(), vec![vec![shared_batch]])
            .expect("shared Arrow buffers are valid MemTable input"),
    );
    base.register_table("points", indexed.clone()).unwrap();
    base.register_table("alias", alias).unwrap();
    let context = attach(
        &base,
        "points",
        &indexed,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    for table in ["points", "alias"] {
        let sql = format!(
            "SELECT id FROM {table} \
             ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 1"
        );
        let plan = explain(&context, &sql).await;
        assert_memtable_fallback(&plan);
    }
}

#[tokio::test]
async fn same_name_replacement_permanently_invalidates_attachment() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let original = vector_mem_table(vec![
        VectorRow::new(1, vec![0.0], "original-zero"),
        VectorRow::new(2, vec![1.0], "original-one"),
    ])
    .unwrap();
    base.register_table("points", original.clone()).unwrap();
    let context = attach(
        &base,
        "points",
        &original,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    base.deregister_table("points").unwrap();
    let replacement = vector_mem_table(vec![
        VectorRow::new(900, vec![9.0], "replacement-nine"),
        VectorRow::new(901, vec![8.0], "replacement-eight"),
    ])
    .unwrap();
    base.register_table("points", replacement).unwrap();

    let sql = "SELECT id FROM points ORDER BY array_distance(embedding, [0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert_memtable_fallback(&plan);
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[901]);
}

#[tokio::test]
async fn changed_memtable_batches_make_attachment_stale() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(rows()).unwrap();
    base.register_table("points", table.clone()).unwrap();
    let context = attach(
        &base,
        "points",
        &table,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    let duplicate_batch = table.batches[0].read().await[0].clone();
    table.batches[0].write().await.push(duplicate_batch);
    let sql = "SELECT id FROM points \
               ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert_memtable_fallback(&plan);
}

#[tokio::test]
async fn attachment_rejects_a_different_registered_instance() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let registered = vector_mem_table(rows()).unwrap();
    let unregistered = vector_mem_table(rows()).unwrap();
    base.register_table("points", registered).unwrap();

    let error = VectorIndexAttachment::try_new(
        &base,
        "points",
        &unregistered,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("not the supplied MemTable instance")
    );
}

#[tokio::test]
async fn arbitrary_schema_and_multi_batch_row_ids_preserve_complete_rows() {
    let context = arbitrary_context().await;
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
    let context = context_with_batches(
        "items",
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
    .await;

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
    let context = arbitrary_context().await;
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
    let count_plan = explain(&context, "SELECT COUNT(*) FROM items").await;
    assert!(!count_plan.contains("VectorIndexScanExec"), "{count_plan}");
    assert!(!count_plan.contains("VectorOrderingExec"), "{count_plan}");

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
    assert_memtable_fallback(&plan);
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
    let context = arbitrary_context().await;
    let wrong_column = "SELECT external_id FROM items \
                        ORDER BY cosine_distance(alternate_vector, [1.0, 0.0, 0.0]) LIMIT 2";
    let plan = explain(&context, wrong_column).await;
    assert_memtable_fallback(&plan);
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
    assert_memtable_fallback(&plan);
    let batches = context
        .sql(no_limit)
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
    assert_eq!(ids.values(), &[101, 102, 103, 104]);
}

#[tokio::test]
async fn rich_schema_matches_only_the_configured_vector_column() {
    let context = rich_schema_context().await;
    let text_sql = "SELECT price, doc_key, active, tenant_id, inventory FROM documents \
                    ORDER BY array_distance(text_embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let text_plan = explain(&context, text_sql).await;
    assert!(text_plan.contains("VectorIndexScanExec"), "{text_plan}");
    assert!(text_plan.contains("ordered=false"), "{text_plan}");
    assert!(text_plan.contains("SortExec: TopK(fetch=2)"), "{text_plan}");
    assert!(!text_plan.contains("DataSourceExec"), "{text_plan}");

    let text = context
        .sql(text_sql)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let text = &text[0];
    assert_eq!(
        text.schema()
            .fields()
            .iter()
            .map(|field| field.name().as_str())
            .collect::<Vec<_>>(),
        ["price", "doc_key", "active", "tenant_id", "inventory"]
    );
    assert_eq!(
        text.column(0)
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .values(),
        &[10.5, 20.25]
    );
    assert_eq!(
        text.column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some("alpha"), Some("beta")]
    );
    assert_eq!(
        text.column(2)
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some(true), Some(false)]
    );
    assert_eq!(
        text.column(3)
            .as_any()
            .downcast_ref::<UInt32Array>()
            .unwrap()
            .values(),
        &[7, 8]
    );
    assert_eq!(
        text.column(4)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap()
            .values(),
        &[4, 3]
    );

    let image_sql = "SELECT price, doc_key, active, tenant_id, inventory FROM documents \
                     ORDER BY array_distance(image_embedding, [1.0, 0.0, 0.0]) LIMIT 2";
    let image_plan = explain(&context, image_sql).await;
    assert_memtable_fallback(&image_plan);
    assert!(
        image_plan.contains("SortExec: TopK(fetch=2)"),
        "{image_plan}"
    );

    let image = context
        .sql(image_sql)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let image = &image[0];
    assert_eq!(
        image
            .column(0)
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .values(),
        &[40.0, 30.75]
    );
    assert_eq!(
        image
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some("delta"), Some("gamma")]
    );
    assert_eq!(
        image
            .column(2)
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some(false), Some(true)]
    );
    assert_eq!(
        image
            .column(3)
            .as_any()
            .downcast_ref::<UInt32Array>()
            .unwrap()
            .values(),
        &[10, 9]
    );
    assert_eq!(
        image
            .column(4)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap()
            .values(),
        &[1, 2]
    );
}

#[tokio::test]
async fn ordered_session_mode_allows_sort_elision() {
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
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
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
    let sql = "SELECT id FROM points WHERE payload = 'north' \
               ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("SortExec"), "{plan}");
    assert!(plan.contains("FilterExec"), "{plan}");
    assert_memtable_fallback(&plan);
}

#[tokio::test]
async fn unsafe_sort_shapes_are_not_lowered() {
    let context = context(Metric::Cosine, IndexConfig::Flat).await;
    for sql in [
        "SELECT id FROM points ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) DESC LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]), id LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, embedding) LIMIT 2",
        "SELECT id FROM points ORDER BY cosine_distance(embedding, [1.0, 0.0]) LIMIT 2",
    ] {
        let plan = explain(&context, sql).await;
        assert!(plan.contains("SortExec"), "{plan}");
        assert_memtable_fallback(&plan);
    }
}

#[tokio::test]
async fn casted_distance_result_keeps_exact_string_order() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(vec![
        VectorRow::new(2, vec![2.0], "two"),
        VectorRow::new(10, vec![10.0], "ten"),
    ])
    .unwrap();
    base.register_table("cast_result", table.clone()).unwrap();
    let context = attach(
        &base,
        "cast_result",
        &table,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    let sql = "SELECT id FROM cast_result \
               ORDER BY CAST(array_distance(embedding, [0.0]) AS VARCHAR) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert_memtable_fallback(&plan);
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[10]);
}

#[tokio::test]
async fn casted_vector_operand_keeps_exact_cast_semantics() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(vec![
        VectorRow::new(1, vec![1.01], "rounds-to-one"),
        VectorRow::new(2, vec![0.98], "rounds-to-zero"),
    ])
    .unwrap();
    base.register_table("cast_vector", table.clone()).unwrap();
    let context = attach(
        &base,
        "cast_vector",
        &table,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    let sql = "SELECT id FROM cast_vector \
               ORDER BY array_distance(CAST(embedding AS INTEGER[]), [0.99]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert_memtable_fallback(&plan);
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[1]);
}

#[tokio::test]
async fn non_round_trippable_float64_query_keeps_exact_f64_order() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(vec![
        VectorRow::new(
            1,
            vec![f32::from_bits(0x3fc0_0002), f32::from_bits(0x3fc0_0001)],
            "exact-nearest",
        ),
        VectorRow::new(
            2,
            vec![1.5_f32, f32::from_bits(0x3fc0_0002)],
            "narrowed-nearest",
        ),
    ])
    .unwrap();
    base.register_table("float64_query", table.clone()).unwrap();
    let context = attach(
        &base,
        "float64_query",
        &table,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    let query = "[1.5000000298023223876953125, 1.4999999701976776123046875]";
    let sql = format!(
        "SELECT id FROM float64_query \
         ORDER BY array_distance(embedding, {query}) LIMIT 1"
    );
    let plan = explain(&context, &sql).await;
    assert_memtable_fallback(&plan);
    let batches = context.sql(&sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(ids.values(), &[1]);

    let forced_exact = format!(
        "SELECT id FROM float64_query \
         ORDER BY array_distance(embedding, {query}) + 0.0 LIMIT 1"
    );
    let batches = context
        .sql(&forced_exact)
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
    assert_eq!(ids.values(), &[1]);

    let exact_query = "SELECT id FROM float64_query \
                       ORDER BY array_distance(embedding, [1.5, 1.5]) LIMIT 1";
    let plan = explain(&context, exact_query).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
}

#[tokio::test]
async fn zero_fetch_never_constructs_a_vector_index_scan() {
    let context = context(Metric::Euclidean, IndexConfig::Flat).await;
    let sql = "SELECT id FROM points \
               ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 0";
    let plan = explain(&context, sql).await;
    assert!(!plan.contains("VectorIndexScanExec"), "{plan}");
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 0);
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
    )
    .await;
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
    )
    .await;
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

#[tokio::test]
async fn user_ids_are_not_engine_row_identity() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(vec![
        VectorRow::new(1, vec![1.0, 0.0], "left"),
        VectorRow::new(1, vec![0.0, 1.0], "right"),
    ])
    .unwrap();
    base.register_table("duplicate_ids", table.clone()).unwrap();
    let context = attach(
        &base,
        "duplicate_ids",
        &table,
        "embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await;

    let sql = "SELECT id, payload FROM duplicate_ids \
               ORDER BY array_distance(embedding, [0.0, 1.0]) LIMIT 2";
    let plan = explain(&context, sql).await;
    assert!(plan.contains("VectorIndexScanExec"), "{plan}");
    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    let payloads = batches[0]
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(ids.values(), &[1, 1]);
    assert_eq!(
        payloads.iter().collect::<Vec<_>>(),
        [Some("right"), Some("left")]
    );
}

#[test]
fn reference_adapter_has_no_custom_snapshot_table_provider() {
    let source = concat!(
        include_str!("../src/lib.rs"),
        include_str!("../src/table.rs"),
        include_str!("../src/attachment.rs"),
        include_str!("../src/optimizer.rs"),
    );
    assert!(!source.contains("trait SnapshotTable"));
    assert!(!source.contains("InMemorySnapshotTable"));
    assert!(!source.contains("struct VectorTable"));
    assert!(!source.contains("struct VectorOrderingExec"));
    assert!(source.contains("pub struct VectorIndexAttachment"));
    assert!(source.contains("struct IndexedSnapshot"));
}

#[test]
fn starter_exposes_the_attachment_api_without_solution_bodies() {
    let source = include_str!("../../../vector-db-starter/datafusion/src/lib.rs");
    assert!(!source.contains("pub struct VectorTable"));
    assert!(source.contains("pub fn vector_mem_table"));
    assert!(source.contains("pub struct VectorIndexAttachment"));
    assert!(source.contains("pub async fn try_new"));
    assert!(source.contains("pub fn with_vector_indexes"));
    assert!(source.contains("_vector_column: impl Into<String>"));
    assert_eq!(source.matches("Day 1:").count(), 6);
    for task in [
        "build the introductory Arrow MemTable",
        "selected vector column",
        "matching MemTable top-k sort",
        "snapshot row id",
        "configured vector column",
        "preserve DataFusion's final sort",
    ] {
        assert!(source.contains(task), "missing solution-free task: {task}");
    }
}

#[tokio::test]
async fn arbitrary_schema_rejects_missing_or_wrong_type_before_index_build() {
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

    let missing = attachment_result(
        vec![batch.clone()],
        "missing",
        Metric::Euclidean,
        invalid_index.clone(),
    )
    .await
    .unwrap_err();
    assert!(
        missing
            .to_string()
            .contains("does not exist or is ambiguous")
    );

    let wrong_type = attachment_result(vec![batch], "score", Metric::Euclidean, invalid_index)
        .await
        .unwrap_err();
    assert!(
        wrong_type
            .to_string()
            .contains("must be FixedSizeList<Float32>")
    );
}

#[tokio::test]
async fn arbitrary_schema_does_not_use_a_user_id_column_as_row_identity() {
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

    let attachment = attachment_result(
        vec![batch],
        "features",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap();
    assert_eq!(attachment.vector_column(), "features");
}

#[tokio::test]
async fn arbitrary_schema_rejects_dimension_drift_and_null_vectors() {
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
    let drift = MemTable::try_new(
        Arc::clone(&schema_2d),
        vec![vec![two_dimensions, three_dimensions]],
    )
    .unwrap_err();
    assert!(drift.to_string().contains("Mismatch between schema"));

    let null_vectors = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, false)),
        2,
        Arc::new(Float32Array::from(vec![1.0, 0.0, 0.0, 1.0])),
        Some(NullBuffer::from(vec![true, false])),
    )
    .unwrap();
    let null_batch =
        RecordBatch::try_new(schema_2d, vec![Arc::new(null_vectors) as ArrayRef]).unwrap();
    let null = attachment_result(
        vec![null_batch],
        "features",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap_err();
    assert!(null.to_string().contains("contains null at row 1"));
}

#[tokio::test]
async fn rich_schema_rejects_a_missing_selected_column() {
    let error = attachment_result(
        rich_schema_batches(),
        "missing_embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("vector column 'missing_embedding' does not exist or is ambiguous")
    );
}

#[tokio::test]
async fn rich_schema_rejects_a_scalar_selected_column() {
    let error = attachment_result(
        rich_schema_batches(),
        "price",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("vector column 'price' must be FixedSizeList<Float32>")
    );
}

#[tokio::test]
async fn rich_schema_rejects_a_zero_width_selected_column() {
    let item = Arc::new(Field::new("item", DataType::Float32, false));
    let schema = Arc::new(Schema::new(vec![Field::new(
        "text_embedding",
        DataType::FixedSizeList(Arc::clone(&item), 0),
        false,
    )]));
    let batch = RecordBatch::try_new(
        schema,
        vec![Arc::new(FixedSizeListArray::new_null(item, 0, 0))],
    )
    .unwrap();
    let error = attachment_result(
        vec![batch],
        "text_embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("vector column 'text_embedding' dimension must be greater than zero")
    );
}

#[tokio::test]
async fn rich_schema_rejects_a_null_selected_value() {
    let item = Arc::new(Field::new("item", DataType::Float32, false));
    let schema = Arc::new(Schema::new(vec![Field::new(
        "text_embedding",
        DataType::FixedSizeList(Arc::clone(&item), 3),
        true,
    )]));
    let vectors = FixedSizeListArray::try_new(
        item,
        3,
        Arc::new(Float32Array::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0])),
        Some(NullBuffer::from(vec![true, false])),
    )
    .unwrap();
    let batch = RecordBatch::try_new(schema, vec![Arc::new(vectors)]).unwrap();
    let error = attachment_result(
        vec![batch],
        "text_embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("vector column 'text_embedding' contains null at row 1")
    );
}

#[tokio::test]
async fn cross_schema_same_name_table_sharing_batches_never_rewrites() {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let indexed = vector_mem_table(rows()).unwrap();
    let shared_batch = indexed.batches[0].read().await[0].clone();

    // A second schema holds a same-named MemTable over the exact same shared
    // Arrow batch as the indexed table. MemTable::scan erases provider
    // identity, so the rewrite-time proof must span every live catalog and
    // schema: this scan is ambiguous and must stay on the exact DataFusion
    // plan even though the attachment's own table still exists. Returning an
    // approximate result from the wrong table would be a real bug.
    let catalog = base.catalog("datafusion").expect("default catalog exists");
    catalog
        .register_schema("other", Arc::new(MemorySchemaProvider::new()))
        .expect("second schema registers");
    let other_schema = catalog.schema("other").expect("other schema resolves");
    let other = Arc::new(
        MemTable::try_new(shared_batch.schema(), vec![vec![shared_batch]])
            .expect("shared Arrow buffers are valid MemTable input"),
    );
    other_schema
        .register_table("points".to_owned(), other.clone())
        .expect("same-named table registers");
    base.register_table("points", indexed.clone()).unwrap();

    let context = attach(
        &base,
        "points",
        &indexed,
        "embedding",
        Metric::Euclidean,
        // A deliberately low-recall index would return the wrong row if the
        // rewrite fired for the ambiguous scan.
        IndexConfig::Hnsw(HnswConfig {
            max_connections: 1,
            ef_construction: 1,
            ef_search: 1,
            max_level: 1,
            seed: 7,
        }),
    )
    .await;

    let sql = "SELECT id FROM other.points \
               ORDER BY array_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 1";
    let plan = explain(&context, sql).await;
    assert_memtable_fallback(&plan);

    let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
    let ids = batches
        .iter()
        .flat_map(|batch| {
            batch
                .column(0)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .iter()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![Some(10)]);
}
