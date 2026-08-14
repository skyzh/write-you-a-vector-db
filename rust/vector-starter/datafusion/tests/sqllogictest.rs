use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use datafusion::arrow::array::{
    ArrayRef, BooleanArray, FixedSizeListArray, Float32Array, Float64Array, Int32Array,
    StringArray, UInt32Array,
};
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::arrow::util::display::array_value_to_string;
use datafusion::common::DataFusionError;
use datafusion::datasource::MemTable;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use datafusion::physical_plan::collect;
use sqllogictest::{AsyncDB, DBOutput, DefaultColumnType, Runner};
use vector_core::{HnswConfig, IndexConfig, IvfFlatConfig, Metric, NswConfig};
use vector_datafusion_starter::{
    VectorIndexAttachment, VectorRow, vector_mem_table, with_vector_indexes,
    with_vector_search_options,
};

struct DataFusionDb {
    context: SessionContext,
}

#[async_trait]
impl AsyncDB for DataFusionDb {
    type Error = DataFusionError;
    type ColumnType = DefaultColumnType;

    async fn run(&mut self, sql: &str) -> Result<DBOutput<Self::ColumnType>, Self::Error> {
        let frame = self.context.sql(sql).await?;
        let task_context = Arc::new(frame.task_ctx());
        let plan = frame.create_physical_plan().await?;
        let schema = plan.schema();
        let batches = collect(plan, task_context).await?;

        if is_explain(sql) {
            return Ok(DBOutput::Rows {
                types: vec![DefaultColumnType::Text],
                rows: vector_plan_rows(&batches)?,
            });
        }

        Ok(DBOutput::Rows {
            types: column_types(&schema),
            rows: string_rows(&batches)?,
        })
    }

    async fn shutdown(&mut self) {}

    fn engine_name(&self) -> &str {
        "DataFusion"
    }

    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

fn is_explain(sql: &str) -> bool {
    sql.trim_start()
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("EXPLAIN"))
}

fn vector_plan_rows(batches: &[RecordBatch]) -> Result<Vec<Vec<String>>, DataFusionError> {
    let rows = string_rows(batches)?;
    let physical_plan = rows
        .iter()
        .find(|row| row.first().is_some_and(|value| value == "physical_plan"))
        .and_then(|row| row.get(1))
        .ok_or_else(|| DataFusionError::Execution("EXPLAIN omitted the physical plan".into()))?;
    let operators = physical_plan
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("SortExec")
                || line.starts_with("VectorIndexScanExec")
                || line.starts_with("FilterExec")
                || line.starts_with("DataSourceExec")
        })
        .map(|line| vec![line.to_owned()])
        .collect::<Vec<_>>();
    if operators.is_empty() {
        return Err(DataFusionError::Execution(
            "EXPLAIN omitted the vector plan operators".into(),
        ));
    }
    Ok(operators)
}

fn column_types(schema: &SchemaRef) -> Vec<DefaultColumnType> {
    schema
        .fields()
        .iter()
        .map(|field| match field.data_type() {
            DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64 => DefaultColumnType::Integer,
            DataType::Float16
            | DataType::Float32
            | DataType::Float64
            | DataType::Decimal32(_, _)
            | DataType::Decimal64(_, _)
            | DataType::Decimal128(_, _)
            | DataType::Decimal256(_, _) => DefaultColumnType::FloatingPoint,
            _ => DefaultColumnType::Text,
        })
        .collect()
}

fn string_rows(batches: &[RecordBatch]) -> Result<Vec<Vec<String>>, DataFusionError> {
    let mut rows = Vec::new();
    for batch in batches {
        for row in 0..batch.num_rows() {
            rows.push(
                batch
                    .columns()
                    .iter()
                    .map(|column| array_value_to_string(column.as_ref(), row))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
    }
    Ok(rows)
}

fn rows() -> Vec<VectorRow> {
    (0..8)
        .map(|id| VectorRow::new(id, vec![id as f32 * 1.25, 1.0, 1.0], format!("point-{id}")))
        .collect()
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

fn rich_schema_batch() -> RecordBatch {
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
    .unwrap()
}

async fn database(config: IndexConfig) -> Result<DataFusionDb, DataFusionError> {
    let base = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = vector_mem_table(rows())?;
    base.register_table("points", table.clone())?;
    let points_attachment = VectorIndexAttachment::try_new(
        &base,
        "points",
        &table,
        "embedding",
        Metric::Euclidean,
        config,
    )
    .await?;
    let batch = rich_schema_batch();
    let documents = Arc::new(MemTable::try_new(batch.schema(), vec![vec![batch]])?);
    base.register_table("documents", documents.clone())?;
    let documents_attachment = VectorIndexAttachment::try_new(
        &base,
        "documents",
        &documents,
        "text_embedding",
        Metric::Euclidean,
        IndexConfig::Flat,
    )
    .await?;
    Ok(DataFusionDb {
        context: with_vector_indexes(&base, vec![points_attachment, documents_attachment]),
    })
}

async fn run_case(filename: &str, config: IndexConfig) {
    let mut runner = Runner::new(move || {
        let config = config.clone();
        async move { database(config).await }
    });
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/slt")
        .join(filename);
    if let Err(error) = runner.run_file_async(&path).await {
        panic!("{}", error.display(false));
    }
    runner.shutdown_async().await;
}

#[tokio::test]
async fn day1_table_and_optimizer_sql() {
    run_case("vector.01-index-match.slt", IndexConfig::Flat).await;
}

#[tokio::test]
async fn day2_ivfflat_sql() {
    run_case(
        "vector.02-ivfflat.slt",
        IndexConfig::IvfFlat(IvfFlatConfig {
            partitions: 3,
            probes: 3,
            iterations: 8,
            seed: 7,
        }),
    )
    .await;
}

#[tokio::test]
async fn day3_nsw_sql() {
    run_case(
        "vector.03-nsw.slt",
        IndexConfig::Nsw(NswConfig {
            max_connections: 4,
            ef_construction: 8,
            ef_search: 8,
        }),
    )
    .await;
}

#[tokio::test]
async fn day4_hnsw_sql() {
    run_case(
        "vector.04-hnsw.slt",
        IndexConfig::Hnsw(HnswConfig {
            max_connections: 4,
            ef_construction: 8,
            ef_search: 8,
            max_level: 4,
            seed: 7,
        }),
    )
    .await;
}
