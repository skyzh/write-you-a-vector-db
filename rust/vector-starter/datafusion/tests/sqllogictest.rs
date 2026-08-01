use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use datafusion::arrow::datatypes::{DataType, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::arrow::util::display::array_value_to_string;
use datafusion::common::DataFusionError;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use datafusion::physical_plan::collect;
use sqllogictest::{AsyncDB, DBOutput, DefaultColumnType, Runner};
use vector_core::{IndexConfig, IvfFlatConfig, Metric, NswConfig};
use vector_datafusion_starter::{VectorRow, VectorTable, with_vector_search_options};

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
                || line.starts_with("VectorScanExec")
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

fn database(config: IndexConfig) -> Result<DataFusionDb, DataFusionError> {
    let context = SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
    let table = VectorTable::try_new(rows(), Metric::Euclidean, config)?;
    context.register_table("points", Arc::new(table))?;
    Ok(DataFusionDb { context })
}

async fn run_case(filename: &str, config: IndexConfig) {
    let mut runner = Runner::new(move || {
        let config = config.clone();
        async move { database(config) }
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
