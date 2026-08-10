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
use datafusion::catalog::SchemaProvider;
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
        plan: Arc<dyn ExecutionPlan>,
        config: &datafusion::common::config::ConfigOptions,
    ) -> DataFusionResult<Transformed<Arc<dyn ExecutionPlan>>> {
        let Some(sort) = plan.downcast_ref::<SortExec>() else {
            return Ok(Transformed::no(plan));
        };
        let Some(fetch) = sort.fetch().filter(|fetch| *fetch > 0) else {
            return Ok(Transformed::no(plan));
        };
        let Some(scan) = sort.input().downcast_ref::<DataSourceExec>() else {
            return Ok(Transformed::no(plan));
        };
        let Some(source) = scan.data_source().downcast_ref::<MemorySourceConfig>() else {
            return Ok(Transformed::no(plan));
        };

        let mut matches = self.attachments.iter().filter_map(|attachment| {
            if !attachment.live_table_matches(source)
                || !attachment.source_matches(source, &scan.schema())
            {
                return None;
            }
            let query = match_vector_order(
                sort.expr(),
                &scan.schema(),
                attachment.snapshot.index.as_ref(),
                &attachment.snapshot.vector_column,
            )?;
            Some((attachment, query))
        });
        let Some((attachment, query)) = matches.next() else {
            return Ok(Transformed::no(plan));
        };
        if matches.next().is_some() {
            return Ok(Transformed::no(plan));
        }

        let ordered = config
            .extensions
            .get::<VectorSearchOptions>()
            .is_some_and(|options| options.ordered);
        let index_scan = VectorIndexScanExec::try_new(
            Arc::clone(&attachment.snapshot),
            source.projection().clone(),
            fetch,
            query.into(),
            ordered,
            sort.expr().to_vec(),
        )?;
        let replacement = index_scan.with_fetch(Some(fetch)).ok_or_else(|| {
            DataFusionError::Internal("positive vector index fetch was rejected".into())
        })?;
        Ok(Transformed::yes(replacement))
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
        // `MemorySourceConfig`. If another registered MemTable exposes the
        // same shared batches, the physical source is ambiguous, so do not
        // guess which table the scan came from.
        self.schema_provider.table_names().into_iter().all(|name| {
            let Some(Ok(Some(provider))) = poll_once(self.schema_provider.table(&name)) else {
                return false;
            };
            Arc::ptr_eq(&provider, &expected)
                || !provider
                    .downcast_ref::<MemTable>()
                    .and_then(|table| memtable_source_matches(table, source))
                    .unwrap_or(false)
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
        let row_count = self.snapshot.indexed_rows.row_locations.len();
        let k = self.fetch.min(row_count);
        self.snapshot
            .index
            .search(&self.query, k)
            .map_err(core_error)?
            .into_iter()
            .map(|neighbor| {
                (neighbor.row < row_count)
                    .then_some(RowId(neighbor.row))
                    .ok_or_else(|| {
                        DataFusionError::Internal(format!(
                            "vector index returned unknown row {}",
                            neighbor.row
                        ))
                    })
            })
            .collect()
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

    fn with_fetch(&self, fetch: Option<usize>) -> Option<Arc<dyn ExecutionPlan>> {
        let fetch = fetch.filter(|fetch| *fetch > 0)?;
        let mut new_scan = self.clone();
        new_scan.fetch = fetch;
        if new_scan.ordered {
            return Some(Arc::new(new_scan));
        }
        // The index selects the candidates; SortExec still owns SQL's ORDER BY contract.
        new_scan.properties = Arc::new(compute_properties(&new_scan.projected_schema, None));
        let ordering = LexOrdering::new(new_scan.ordering.clone())?;
        let sort = SortExec::new(ordering, Arc::new(new_scan)).with_fetch(Some(fetch));
        Some(Arc::new(sort))
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
    order: &[PhysicalSortExpr],
    schema: &Schema,
    index: &dyn VectorIndex,
    vector_column: &str,
) -> Option<Vec<f32>> {
    let [sort] = order else {
        return None;
    };
    let function = sort.expr.as_ref().downcast_ref::<ScalarFunctionExpr>()?;
    let expected_descending = index.metric() == Metric::Dot;
    if sort.options.descending != expected_descending
        || metric_for_function(function.name())? != index.metric()
    {
        return None;
    }
    let [left, right] = function.args() else {
        return None;
    };
    let (column, literal) = match (
        match_vector_column(left.as_ref(), schema),
        right.as_ref().downcast_ref::<Literal>(),
    ) {
        (Some(column), Some(literal)) => (column, literal),
        _ => match (
            match_vector_column(right.as_ref(), schema),
            left.as_ref().downcast_ref::<Literal>(),
        ) {
            (Some(column), Some(literal)) => (column, literal),
            _ => return None,
        },
    };
    if column.index() >= schema.fields().len()
        || schema.field(column.index()).name() != vector_column
    {
        return None;
    }
    let query = scalar_vector(literal.value())?;
    if query.len() != index.dataset().dimension()
        || query.iter().any(|value| !value.is_finite())
        || (index.metric() == Metric::Cosine && query.iter().all(|value| *value == 0.0))
    {
        return None;
    }
    Some(query)
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

#[cfg(test)]
mod snapshot_tests {
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
    fn float64_query_admission_requires_bit_exact_f32_round_trip() {
        let exact = Float64Array::from(vec![0.0, -0.0, 1.5]);
        let narrowed = primitive_vector(&exact).unwrap();
        assert_eq!(narrowed[0].to_bits(), 0.0_f32.to_bits());
        assert_eq!(narrowed[1].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(narrowed[2], 1.5_f32);

        let precision_loss = Float64Array::from(vec![1.500_000_029_802_322_4]);
        assert!(primitive_vector(&precision_loss).is_none());
        let non_finite = Float64Array::from(vec![f64::INFINITY]);
        assert!(primitive_vector(&non_finite).is_none());
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
