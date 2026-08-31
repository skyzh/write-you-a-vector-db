use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};

use datafusion::arrow::array::{Array, ArrayRef, FixedSizeListArray, Float32Array};
use datafusion::arrow::compute::concat_batches;
use datafusion::arrow::datatypes::{DataType, SchemaRef};
use datafusion::arrow::record_batch::{RecordBatch, RecordBatchOptions};
use datafusion::catalog::{CatalogProviderList, SchemaProvider};
use datafusion::common::{DataFusionError, Result as DataFusionResult, TableReference};
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::datasource::{MemTable, TableProvider};
use datafusion::execution::context::SessionContext;
use datafusion::physical_plan::project_schema;
use vector_core::{Dataset, IndexConfig, Metric, VectorIndex};

use crate::core_error;

/// A vector index attached outside one ordinary registered `MemTable`.
#[derive(Debug, Clone)]
pub struct VectorIndexAttachment {
    pub(crate) snapshot: Arc<VectorIndexSnapshot>,
    schema_provider: Arc<dyn SchemaProvider>,
    catalog_list: Arc<dyn CatalogProviderList>,
    table_name: Arc<str>,
    table_provider: Weak<dyn datafusion::datasource::TableProvider>,
}

#[derive(Debug)]
pub(crate) struct VectorIndexSnapshot {
    pub(crate) indexed_rows: Arc<IndexedSnapshot>,
    source_partitions: Arc<[Vec<RecordBatch>]>,
    pub(crate) vector_column: Arc<str>,
    pub(crate) index: Arc<dyn VectorIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowId(pub(crate) usize);

/// Indexed view of the immutable source batches.
///
/// DataFusion's `MemTable` owns its own lightweight `RecordBatch` handles for
/// exact scans. This snapshot owns another set of handles plus stable ordinal
/// locators for index hits. Both share the same Arrow column buffers.
#[derive(Debug)]
pub(crate) struct IndexedSnapshot {
    pub(crate) schema: SchemaRef,
    batches: Arc<[RecordBatch]>,
    pub(crate) row_locations: Arc<[RowLocation]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowLocation {
    batch: usize,
    row: usize,
}

impl IndexedSnapshot {
    pub(crate) fn lookup(
        &self,
        row_ids: &[RowId],
        projection: Option<&Vec<usize>>,
    ) -> DataFusionResult<RecordBatch> {
        let projected_schema = project_schema(&self.schema, projection)?;
        let projected_columns = projection
            .cloned()
            .unwrap_or_else(|| (0..self.schema.fields().len()).collect());
        let fragments = row_ids
            .iter()
            .map(|row_id| {
                let location = self.row_locations.get(row_id.0).ok_or_else(|| {
                    DataFusionError::Internal(format!(
                        "row id {} does not exist in the snapshot table",
                        row_id.0
                    ))
                })?;
                let batch = self.batches.get(location.batch).ok_or_else(|| {
                    DataFusionError::Internal(format!(
                        "row id {} references unknown batch {}",
                        row_id.0, location.batch
                    ))
                })?;
                if location.row >= batch.num_rows() {
                    return Err(DataFusionError::Internal(format!(
                        "row id {} references row {} in a {}-row batch",
                        row_id.0,
                        location.row,
                        batch.num_rows()
                    )));
                }
                let columns = projected_columns
                    .iter()
                    .map(|column| batch.column(*column).slice(location.row, 1))
                    .collect::<Vec<ArrayRef>>();
                RecordBatch::try_new_with_options(
                    Arc::clone(&projected_schema),
                    columns,
                    &RecordBatchOptions::new().with_row_count(Some(1)),
                )
                .map_err(DataFusionError::from)
            })
            .collect::<DataFusionResult<Vec<_>>>()?;

        if fragments.is_empty() {
            return Ok(RecordBatch::new_empty(projected_schema));
        }
        concat_batches(&projected_schema, &fragments).map_err(DataFusionError::from)
    }
}

impl VectorIndexAttachment {
    /// Attach one vector index to an already registered ordinary `MemTable`.
    ///
    /// The attachment snapshots the table's Arrow batches and binds the exact
    /// registered provider identity. A later table replacement, batch change,
    /// schema change, or physical source mismatch fails closed to DataFusion's
    /// exact plan.
    pub async fn try_new(
        context: &SessionContext,
        table_ref: impl Into<TableReference>,
        table: &Arc<MemTable>,
        vector_column: impl Into<String>,
        metric: Metric,
        index: IndexConfig,
    ) -> DataFusionResult<Self> {
        let table_ref = table_ref.into();
        let table_name = table_ref.table().to_owned();
        let schema_provider = context.state().schema_for_ref(table_ref.clone())?;
        let catalog_list = Arc::clone(context.state().catalog_list());
        let current_provider = context.table_provider(table_ref).await?;
        let table_provider: Arc<dyn datafusion::datasource::TableProvider> = table.clone();
        if !Arc::ptr_eq(&current_provider, &table_provider) {
            return Err(DataFusionError::Plan(format!(
                "registered table '{table_name}' is not the supplied MemTable instance"
            )));
        }

        let mut source_partitions = Vec::with_capacity(table.batches.len());
        for partition in &table.batches {
            source_partitions.push(partition.read().await.clone());
        }
        let batches = source_partitions
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        let vector_column = vector_column.into();
        let Some(first_batch) = batches.first() else {
            return Err(DataFusionError::Plan(
                "a vector index requires at least one record batch".into(),
            ));
        };
        let schema = first_batch.schema();
        if table.schema().as_ref() != schema.as_ref() {
            return Err(DataFusionError::Plan(
                "MemTable schema does not match its indexed record batches".into(),
            ));
        }
        for (batch_idx, batch) in batches.iter().enumerate().skip(1) {
            if batch.schema_ref().as_ref() != schema.as_ref() {
                return Err(DataFusionError::Plan(format!(
                    "record batch {batch_idx} does not match the indexed table schema"
                )));
            }
        }

        let column_idx = schema.index_of(&vector_column).map_err(|_| {
            DataFusionError::Plan(format!(
                "vector column '{vector_column}' does not exist or is ambiguous"
            ))
        })?;
        let field = schema.field(column_idx);
        let DataType::FixedSizeList(item_field, dimension) = field.data_type() else {
            return Err(DataFusionError::Plan(format!(
                "vector column '{vector_column}' must be FixedSizeList<Float32>, got {}",
                field.data_type()
            )));
        };
        if item_field.data_type() != &DataType::Float32 {
            return Err(DataFusionError::Plan(format!(
                "vector column '{vector_column}' must be FixedSizeList<Float32>, got {}",
                field.data_type()
            )));
        }
        if *dimension <= 0 {
            return Err(DataFusionError::Plan(format!(
                "vector column '{vector_column}' dimension must be greater than zero"
            )));
        }

        let row_count = batches.iter().map(RecordBatch::num_rows).sum();
        let mut vectors = Vec::with_capacity(row_count);
        let mut row_locations = Vec::with_capacity(row_count);
        let mut vector_ordinal = 0;
        for (batch_idx, batch) in batches.iter().enumerate() {
            let vectors_array = batch
                .column(column_idx)
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or_else(|| {
                    DataFusionError::Plan(format!(
                        "vector column '{vector_column}' must be FixedSizeList<Float32>"
                    ))
                })?;
            for row in 0..batch.num_rows() {
                if vectors_array.is_null(row) {
                    return Err(DataFusionError::Plan(format!(
                        "vector column '{vector_column}' contains null at row {vector_ordinal}"
                    )));
                }
                let values = vectors_array.value(row);
                let values = values
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| {
                        DataFusionError::Plan(format!(
                            "vector column '{vector_column}' must be FixedSizeList<Float32>"
                        ))
                    })?;
                if values.null_count() != 0 {
                    return Err(DataFusionError::Plan(format!(
                        "vector column '{vector_column}' contains a null element at row {vector_ordinal}"
                    )));
                }
                vectors.push(values.values().to_vec());
                row_locations.push(RowLocation {
                    batch: batch_idx,
                    row,
                });
                vector_ordinal += 1;
            }
        }

        let dataset = Dataset::try_new(vectors).map_err(core_error)?;
        let index = index.build(dataset, metric).map_err(core_error)?;
        let indexed_rows = Arc::new(IndexedSnapshot {
            schema: Arc::clone(&schema),
            batches: batches.clone().into(),
            row_locations: row_locations.into(),
        });

        Ok(Self {
            snapshot: Arc::new(VectorIndexSnapshot {
                indexed_rows,
                source_partitions: source_partitions.into(),
                vector_column: vector_column.into(),
                index,
            }),
            schema_provider,
            catalog_list,
            table_name: table_name.into(),
            table_provider: Arc::downgrade(&table_provider),
        })
    }

    pub fn index_kind(&self) -> &'static str {
        self.snapshot.index.kind()
    }

    pub fn metric(&self) -> Metric {
        self.snapshot.index.metric()
    }

    pub fn vector_column(&self) -> &str {
        &self.snapshot.vector_column
    }
}

/// Clone a session around its existing catalogs and install one physical
/// vector-index rule. The registered providers remain ordinary `MemTable`s.
impl VectorIndexAttachment {
    pub(crate) fn live_table_matches(&self, source: &MemorySourceConfig) -> bool {
        let Some(expected) = self.table_provider.upgrade() else {
            return false;
        };
        let Some(result) = poll_once(self.schema_provider.table(&self.table_name)) else {
            return false;
        };
        let Ok(Some(current)) = result else {
            return false;
        };
        if !Arc::ptr_eq(&current, &expected)
            || !current
                .downcast_ref::<MemTable>()
                .and_then(|table| memtable_source_matches(table, source))
                .unwrap_or(false)
        {
            return false;
        }

        // `MemTable::scan` intentionally erases provider identity into a
        // `MemorySourceConfig`. If any other registered MemTable anywhere in
        // the live catalog list exposes the same shared batches, the physical
        // source is ambiguous (a same-named table in another schema can even
        // shadow this one), so do not guess which table the scan came from.
        // `Arc::ptr_eq` is the exact provider identity: a genuine alias of the
        // same provider still short-circuits, while a distinct provider that
        // happens to share Arrow buffers fails closed.
        self.catalog_list
            .catalog_names()
            .into_iter()
            .all(|catalog_name| {
                let Some(catalog) = self.catalog_list.catalog(&catalog_name) else {
                    return false;
                };
                catalog.schema_names().into_iter().all(|schema_name| {
                    let Some(schema) = catalog.schema(&schema_name) else {
                        return false;
                    };
                    schema.table_names().into_iter().all(|name| {
                        let Some(Ok(Some(provider))) = poll_once(schema.table(&name)) else {
                            return false;
                        };
                        Arc::ptr_eq(&provider, &expected)
                            || !provider
                                .downcast_ref::<MemTable>()
                                .and_then(|table| memtable_source_matches(table, source))
                                .unwrap_or(false)
                    })
                })
            })
    }

    pub(crate) fn source_matches(
        &self,
        source: &MemorySourceConfig,
        scan_schema: &SchemaRef,
    ) -> bool {
        source.original_schema().as_ref() == self.snapshot.indexed_rows.schema.as_ref()
            && same_partitions(source.partitions(), &self.snapshot.source_partitions)
            && project_schema(
                &self.snapshot.indexed_rows.schema,
                source.projection().as_ref(),
            )
            .is_ok_and(|expected| expected == *scan_schema)
    }
}

fn memtable_source_matches(table: &MemTable, source: &MemorySourceConfig) -> Option<bool> {
    if table.batches.len() != source.partitions().len() {
        return Some(false);
    }
    for (partition, actual) in table.batches.iter().zip(source.partitions()) {
        let expected = partition.try_read().ok()?;
        if expected.len() != actual.len()
            || !expected
                .iter()
                .zip(actual)
                .all(|(expected, actual)| same_batch_identity(actual, expected))
        {
            return Some(false);
        }
    }
    Some(true)
}

fn same_partitions(actual: &[Vec<RecordBatch>], expected: &[Vec<RecordBatch>]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.len() == expected.len()
                && actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| same_batch_identity(actual, expected))
        })
}

fn same_batch_identity(actual: &RecordBatch, expected: &RecordBatch) -> bool {
    actual.num_rows() == expected.num_rows()
        && actual.num_columns() == expected.num_columns()
        && actual
            .columns()
            .iter()
            .zip(expected.columns())
            .all(|(actual, expected)| Arc::ptr_eq(actual, expected))
}

fn poll_once<F: Future>(future: F) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

#[cfg(test)]
mod snapshot_tests {
    use datafusion::arrow::array::{Int32Array, StringArray};
    use datafusion::arrow::datatypes::{DataType, Field, Schema};

    use super::*;

    fn indexed_snapshot() -> IndexedSnapshot {
        let schema = Arc::new(Schema::new(vec![
            Field::new("label", DataType::Utf8, false),
            Field::new("score", DataType::Int32, false),
        ]));
        let batches = vec![
            RecordBatch::try_new(
                Arc::clone(&schema),
                vec![
                    Arc::new(StringArray::from(vec!["a", "b"])),
                    Arc::new(Int32Array::from(vec![10, 20])),
                ],
            )
            .unwrap(),
            RecordBatch::try_new(
                Arc::clone(&schema),
                vec![
                    Arc::new(StringArray::from(vec!["c", "d"])),
                    Arc::new(Int32Array::from(vec![30, 40])),
                ],
            )
            .unwrap(),
        ];
        IndexedSnapshot {
            schema,
            batches: batches.into(),
            row_locations: vec![
                RowLocation { batch: 0, row: 0 },
                RowLocation { batch: 0, row: 1 },
                RowLocation { batch: 1, row: 0 },
                RowLocation { batch: 1, row: 1 },
            ]
            .into(),
        }
    }

    #[test]
    fn snapshot_lookup_preserves_multibatch_order_and_projection() {
        let table = indexed_snapshot();
        let projection = vec![1, 0];
        let batch = table
            .lookup(&[RowId(3), RowId(0)], Some(&projection))
            .unwrap();

        assert_eq!(batch.schema().field(0).name(), "score");
        assert_eq!(batch.schema().field(1).name(), "label");
        let scores = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        let labels = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(scores.values(), &[40, 10]);
        assert_eq!(labels.iter().collect::<Vec<_>>(), [Some("d"), Some("a")]);
    }

    #[test]
    fn snapshot_lookup_rejects_unknown_row_ids() {
        let error = indexed_snapshot().lookup(&[RowId(4)], None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("row id 4 does not exist in the snapshot table"),
            "{error}"
        );
    }

    #[test]
    fn empty_projection_preserves_selected_row_count() {
        let batch = indexed_snapshot()
            .lookup(&[RowId(3), RowId(0)], Some(&vec![]))
            .unwrap();
        assert_eq!(batch.num_columns(), 0);
        assert_eq!(batch.num_rows(), 2);
    }

    #[test]
    fn source_identity_requires_shared_arrow_buffers() {
        let snapshot = indexed_snapshot();
        let shared = snapshot.batches[0].clone();
        assert!(same_batch_identity(&shared, &snapshot.batches[0]));

        let copied = RecordBatch::try_new(
            snapshot.batches[0].schema(),
            vec![
                Arc::new(StringArray::from(vec!["a", "b"])),
                Arc::new(Int32Array::from(vec![10, 20])),
            ],
        )
        .unwrap();
        assert!(!same_batch_identity(&copied, &snapshot.batches[0]));
    }
}
