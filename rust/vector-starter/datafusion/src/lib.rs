#![allow(dead_code, unused_imports)]
use std::fmt::{self, Formatter};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};

use datafusion::arrow::array::{
    Array, ArrayRef, FixedSizeListArray, Float32Array, Float64Array, Int32Array, Int64Array,
    StringArray, UInt64Array,
};
use datafusion::arrow::compute::concat_batches;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::{RecordBatch, RecordBatchOptions};
use datafusion::catalog::{CatalogProviderList, SchemaProvider};
use datafusion::common::config::ConfigExtension;
use datafusion::common::tree_node::{Transformed, TreeNode};
use datafusion::common::{
    DataFusionError, Result as DataFusionResult, ScalarValue, TableReference, extensions_options,
};
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::datasource::source::DataSourceExec;
use datafusion::datasource::{MemTable, TableProvider};
use datafusion::execution::SessionStateBuilder;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::{SessionContext, TaskContext};
use datafusion::physical_expr::expressions::{CastExpr, Column, Literal};
use datafusion::physical_expr::{
    EquivalenceProperties, LexOrdering, PhysicalExpr, ScalarFunctionExpr,
};
use datafusion::physical_optimizer::PhysicalOptimizerRule;
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::expressions::PhysicalSortExpr;
use datafusion::physical_plan::memory::MemoryStream;
use datafusion::physical_plan::sorts::sort::SortExec;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning, PlanProperties,
    SendableRecordBatchStream, project_schema,
};
use vector_core::{Dataset, IndexConfig, Metric, VectorIndex};

const EMBEDDING_COLUMN: &str = "embedding";

extensions_options! {
    /// Session options for vector index execution.
    pub struct VectorSearchOptions {
        /// Whether the vector index executor guarantees distance-ordered output.
        pub ordered: bool, default = false
    }
}

impl ConfigExtension for VectorSearchOptions {
    const PREFIX: &'static str = "vector_search";
}

/// Register the `vector_search` namespace so SQL `SET` statements can update it.
pub fn with_vector_search_options(mut config: SessionConfig) -> SessionConfig {
    config
        .options_mut()
        .extensions
        .insert(VectorSearchOptions::default());
    config
}

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

/// A vector index attached outside one ordinary registered `MemTable`.
#[derive(Debug, Clone)]
pub struct VectorIndexAttachment {
    snapshot: Arc<VectorIndexSnapshot>,
    schema_provider: Arc<dyn SchemaProvider>,
    catalog_list: Arc<dyn CatalogProviderList>,
    table_name: Arc<str>,
    table_provider: Weak<dyn datafusion::datasource::TableProvider>,
}

#[derive(Debug)]
struct VectorIndexSnapshot {
    indexed_rows: Arc<IndexedSnapshot>,
    source_partitions: Arc<[Vec<RecordBatch>]>,
    vector_column: Arc<str>,
    index: Arc<dyn VectorIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowId(usize);

/// Indexed view of the immutable source batches.
///
/// DataFusion's `MemTable` owns its own lightweight `RecordBatch` handles for
/// exact scans. This snapshot owns another set of handles plus stable ordinal
/// locators for index hits. Both share the same Arrow column buffers.
#[derive(Debug)]
struct IndexedSnapshot {
    schema: SchemaRef,
    batches: Arc<[RecordBatch]>,
    row_locations: Arc<[RowLocation]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowLocation {
    batch: usize,
    row: usize,
}

impl IndexedSnapshot {
    fn lookup(
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

/// Build the course's three-column example as an ordinary DataFusion `MemTable`.
pub fn vector_mem_table(_rows: Vec<VectorRow>) -> DataFusionResult<Arc<MemTable>> {
    todo!("Chapter 1: validate the simple rows and build the introductory Arrow MemTable")
}

impl VectorIndexAttachment {
    /// Attach one vector index to an already registered ordinary `MemTable`.
    ///
    /// The attachment snapshots the table's Arrow batches and binds the exact
    /// registered provider identity. A later table replacement, batch change,
    /// schema change, or physical source mismatch fails closed to DataFusion's
    /// exact plan.
    pub async fn try_new(
        _context: &SessionContext,
        _table_ref: impl Into<TableReference>,
        _table: &Arc<MemTable>,
        _vector_column: impl Into<String>,
        _metric: Metric,
        _index: IndexConfig,
    ) -> DataFusionResult<Self> {
        todo!(
            "Chapter 1: bind the registered MemTable, selected vector column, snapshot rows, and core index"
        )
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
pub fn with_vector_indexes(
    context: &SessionContext,
    attachments: Vec<VectorIndexAttachment>,
) -> SessionContext {
    let state = SessionStateBuilder::new_from_existing(context.state())
        .with_physical_optimizer_rule(Arc::new(VectorIndexOptimizer {
            attachments: attachments.into(),
        }))
        .build();
    SessionContext::new_with_state(state)
}

#[derive(Debug)]
struct VectorIndexOptimizer {
    attachments: Arc<[VectorIndexAttachment]>,
}

impl PhysicalOptimizerRule for VectorIndexOptimizer {
    fn optimize(
        &self,
        plan: Arc<dyn ExecutionPlan>,
        config: &datafusion::common::config::ConfigOptions,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        plan.transform_up(|plan| self.rewrite_sort(plan, config))
            .map(|transformed| transformed.data)
    }

    fn name(&self) -> &str {
        "vector_index_topk"
    }

    fn schema_check(&self) -> bool {
        true
    }
}

impl VectorIndexOptimizer {
    fn rewrite_sort(
        &self,
        _plan: Arc<dyn ExecutionPlan>,
        _config: &datafusion::common::config::ConfigOptions,
    ) -> DataFusionResult<Transformed<Arc<dyn ExecutionPlan>>> {
        todo!(
            "Chapter 1: replace only a matching MemTable top-k sort with the attached vector index"
        )
    }
}

impl VectorIndexAttachment {
    fn live_table_matches(&self, source: &MemorySourceConfig) -> bool {
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

    fn source_matches(&self, source: &MemorySourceConfig, scan_schema: &SchemaRef) -> bool {
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

#[derive(Debug, Clone)]
struct VectorIndexScanExec {
    snapshot: Arc<VectorIndexSnapshot>,
    projection: Option<Vec<usize>>,
    projected_schema: SchemaRef,
    fetch: usize,
    query: Arc<[f32]>,
    ordered: bool,
    ordering: Vec<PhysicalSortExpr>,
    properties: Arc<PlanProperties>,
}

impl VectorIndexScanExec {
    fn try_new(
        snapshot: Arc<VectorIndexSnapshot>,
        projection: Option<Vec<usize>>,
        fetch: usize,
        query: Arc<[f32]>,
        ordered: bool,
        ordering: Vec<PhysicalSortExpr>,
    ) -> DataFusionResult<Self> {
        if fetch == 0 {
            return Err(DataFusionError::Plan(
                "vector index scan requires a positive fetch".into(),
            ));
        }
        let projected_schema = project_schema(&snapshot.indexed_rows.schema, projection.as_ref())?;
        let properties = compute_properties(&projected_schema, Some(&ordering));
        Ok(Self {
            snapshot,
            projection,
            projected_schema,
            fetch,
            query,
            ordered,
            ordering,
            properties: Arc::new(properties),
        })
    }

    fn selected_rows(&self) -> DataFusionResult<Vec<RowId>> {
        todo!("Chapter 1: search the selected index and validate every returned snapshot row id")
    }

    fn output_batch(&self, rows: &[RowId]) -> DataFusionResult<RecordBatch> {
        self.snapshot
            .indexed_rows
            .lookup(rows, self.projection.as_ref())
    }
}

impl DisplayAs for VectorIndexScanExec {
    fn fmt_as(&self, format: DisplayFormatType, f: &mut Formatter<'_>) -> fmt::Result {
        match format {
            DisplayFormatType::Default | DisplayFormatType::Verbose => write!(
                f,
                "VectorIndexScanExec: index={}, metric={:?}, query_dim={}, fetch=Some({}), ordered={}",
                self.snapshot.index.kind(),
                self.snapshot.index.metric(),
                self.query.len(),
                self.fetch,
                self.ordered
            ),
            DisplayFormatType::TreeRender => write!(f, "{}", self.name()),
        }
    }
}

impl ExecutionPlan for VectorIndexScanExec {
    fn name(&self) -> &str {
        "VectorIndexScanExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        Vec::new()
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        if children.is_empty() {
            Ok(self)
        } else {
            Err(DataFusionError::Internal(
                "VectorIndexScanExec is a leaf plan".into(),
            ))
        }
    }

    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> DataFusionResult<SendableRecordBatchStream> {
        if partition != 0 {
            return Err(DataFusionError::Execution(format!(
                "VectorIndexScanExec has one partition, requested {partition}"
            )));
        }
        let rows = self.selected_rows()?;
        let batch = self.output_batch(&rows)?;
        Ok(Box::pin(MemoryStream::try_new(
            vec![batch],
            Arc::clone(&self.projected_schema),
            None,
        )?))
    }

    fn with_fetch(&self, _fetch: Option<usize>) -> Option<Arc<dyn ExecutionPlan>> {
        todo!("Chapter 1: preserve DataFusion's final sort unless ordered index output is enabled")
    }

    fn fetch(&self) -> Option<usize> {
        Some(self.fetch)
    }
}

fn compute_properties(schema: &SchemaRef, ordering: Option<&[PhysicalSortExpr]>) -> PlanProperties {
    let equivalence = match ordering {
        Some(ordering) => {
            EquivalenceProperties::new_with_orderings(Arc::clone(schema), [ordering.to_vec()])
        }
        None => EquivalenceProperties::new(Arc::clone(schema)),
    };
    PlanProperties::new(
        equivalence,
        Partitioning::UnknownPartitioning(1),
        EmissionType::Incremental,
        Boundedness::Bounded,
    )
}

fn match_vector_order(
    _order: &[PhysicalSortExpr],
    _schema: &Schema,
    _index: &dyn VectorIndex,
    _vector_column: &str,
) -> Option<Vec<f32>> {
    todo!(
        "Chapter 1: match function, direction, configured vector column, literal, metric, and dimension"
    )
}

fn match_vector_column<'a>(
    expression: &'a dyn PhysicalExpr,
    schema: &Schema,
) -> Option<&'a Column> {
    if let Some(column) = expression.downcast_ref::<Column>() {
        return Some(column);
    }

    // DataFusion widens a FixedSizeList<Float32> argument to List<Float64>
    // for its distance functions. Accept exactly that planner-generated cast,
    // but no recursively wrapped or semantically different user cast.
    let cast = expression.downcast_ref::<CastExpr>()?;
    let column = cast.expr().as_ref().downcast_ref::<Column>()?;
    let source = schema.fields().get(column.index())?.data_type();
    let source_is_vector = matches!(
        source,
        DataType::FixedSizeList(item, _) if item.data_type() == &DataType::Float32
    );
    let target_is_distance_input = matches!(
        cast.cast_type(),
        DataType::List(item) if item.data_type() == &DataType::Float64
    );
    (source_is_vector && target_is_distance_input).then_some(column)
}

fn metric_for_function(name: &str) -> Option<Metric> {
    match name {
        "array_distance" | "list_distance" => Some(Metric::Euclidean),
        "cosine_distance" => Some(Metric::Cosine),
        "inner_product" | "dot_product" => Some(Metric::Dot),
        _ => None,
    }
}

fn scalar_vector(value: &ScalarValue) -> Option<Vec<f32>> {
    let values = match value {
        ScalarValue::List(array) if array.len() == 1 && !array.is_null(0) => array.value(0),
        ScalarValue::LargeList(array) if array.len() == 1 && !array.is_null(0) => array.value(0),
        ScalarValue::FixedSizeList(array) if array.len() == 1 && !array.is_null(0) => {
            array.value(0)
        }
        _ => return None,
    };
    primitive_vector(values.as_ref())
}

fn primitive_vector(values: &dyn Array) -> Option<Vec<f32>> {
    if values.null_count() != 0 {
        return None;
    }
    if let Some(values) = values.as_any().downcast_ref::<Float32Array>() {
        return Some(values.values().to_vec());
    }
    if let Some(values) = values.as_any().downcast_ref::<Float64Array>() {
        return values
            .values()
            .iter()
            .copied()
            .map(exact_f32_query_value)
            .collect();
    }
    if let Some(values) = values.as_any().downcast_ref::<Int32Array>() {
        return Some(values.values().iter().map(|value| *value as f32).collect());
    }
    if let Some(values) = values.as_any().downcast_ref::<Int64Array>() {
        return Some(values.values().iter().map(|value| *value as f32).collect());
    }
    None
}

/// Admit a widened Float64 query value only when the index's Float32 query
/// boundary preserves the exact value, including the sign bit of zero.
fn exact_f32_query_value(value: f64) -> Option<f32> {
    let narrowed = value as f32;
    (value.is_finite() && (narrowed as f64).to_bits() == value.to_bits()).then_some(narrowed)
}

fn core_error(error: vector_core::VectorError) -> DataFusionError {
    DataFusionError::Plan(error.to_string())
}
