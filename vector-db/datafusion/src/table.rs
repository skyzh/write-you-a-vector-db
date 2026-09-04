use std::sync::Arc;

use datafusion::arrow::array::{FixedSizeListArray, Float32Array, StringArray, UInt64Array};
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::{DataFusionError, Result as DataFusionResult};
use datafusion::datasource::MemTable;
use vector_core::Dataset;

use crate::core_error;

const EMBEDDING_COLUMN: &str = "embedding";

#[derive(Debug, Clone)]
pub struct VectorRow {
    pub id: u64,
    pub embedding: Vec<f32>,
    pub payload: String,
}

impl VectorRow {
    pub fn new(id: u64, embedding: Vec<f32>, payload: impl Into<String>) -> Self {
        Self {
            id,
            embedding,
            payload: payload.into(),
        }
    }
}

/// Build the course's three-column example as an ordinary DataFusion `MemTable`.
pub fn vector_mem_table(rows: Vec<VectorRow>) -> DataFusionResult<Arc<MemTable>> {
    let dataset = Dataset::try_new(
        rows.iter()
            .map(|row| row.embedding.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(core_error)?;
    let dimension = i32::try_from(dataset.dimension())
        .map_err(|_| DataFusionError::Plan("vector dimension exceeds i32::MAX".into()))?;
    let item_field = Arc::new(Field::new("item", DataType::Float32, false));
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("payload", DataType::Utf8, false),
        Field::new(
            EMBEDDING_COLUMN,
            DataType::FixedSizeList(Arc::clone(&item_field), dimension),
            false,
        ),
    ]));
    let embedding_values = rows
        .iter()
        .flat_map(|row| row.embedding.iter().copied())
        .collect::<Vec<_>>();
    let embeddings = FixedSizeListArray::try_new(
        item_field,
        dimension,
        Arc::new(Float32Array::from(embedding_values)),
        None,
    )?;
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(UInt64Array::from(
                rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.payload.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(embeddings),
        ],
    )?;
    Ok(Arc::new(MemTable::try_new(schema, vec![vec![batch]])?))
}
