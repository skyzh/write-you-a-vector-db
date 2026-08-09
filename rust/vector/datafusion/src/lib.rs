use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::arrow::array::{
    Array, ArrayRef, FixedSizeListArray, Float32Array, Float64Array, Int32Array, Int64Array,
    StringArray, UInt64Array,
};
use datafusion::arrow::compute::concat_batches;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::{RecordBatch, RecordBatchOptions};
use datafusion::catalog::Session;
use datafusion::common::config::ConfigExtension;
use datafusion::common::{
    DataFusionError, Result as DataFusionResult, ScalarValue, extensions_options,
};
use datafusion::datasource::{TableProvider, TableType};
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::TaskContext;
use datafusion::logical_expr::Expr;
use datafusion::physical_expr::expressions::{CastExpr, Column, Literal};
use datafusion::physical_expr::{
    EquivalenceProperties, LexOrdering, PhysicalExpr, ScalarFunctionExpr,
};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::expressions::PhysicalSortExpr;
use datafusion::physical_plan::memory::MemoryStream;
use datafusion::physical_plan::sorts::sort::SortExec;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning, PlanProperties,
    SendableRecordBatchStream, SortOrderPushdownResult, project_schema,
};
use vector_core::{Dataset, FlatIndex, IndexConfig, Metric, VectorIndex};

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

#[derive(Debug, Clone)]
pub struct VectorTable {
    snapshot: Arc<VectorTableSnapshot>,
}

#[derive(Debug)]
struct VectorTableSnapshot {
    schema: SchemaRef,
    batches: Arc<[RecordBatch]>,
    row_ids: Arc<[RowId]>,
    vector_column: Arc<str>,
    index: Arc<dyn VectorIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowId {
    batch: usize,
    row: usize,
}

impl VectorTable {
    pub fn try_new(
        rows: Vec<VectorRow>,
        metric: Metric,
        index: IndexConfig,
    ) -> DataFusionResult<Self> {
        let mut ids = HashSet::with_capacity(rows.len());
        for row in &rows {
            if !ids.insert(row.id) {
                return Err(DataFusionError::Plan(format!(
                    "duplicate vector id {}",
                    row.id
                )));
            }
        }

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

        Self::try_new_batch(batch, EMBEDDING_COLUMN, metric, index)
    }

    /// Build an immutable vector table from one arbitrary-schema Arrow batch.
    ///
    /// Only `vector_column` is copied into the index. Every search result is
    /// mapped back to the original batch through an engine-owned row locator.
    pub fn try_new_batch(
        batch: RecordBatch,
        vector_column: impl Into<String>,
        metric: Metric,
        index: IndexConfig,
    ) -> DataFusionResult<Self> {
        Self::try_new_batches(vec![batch], vector_column, metric, index)
    }

    /// Build an immutable vector table from arbitrary-schema Arrow batches.
    ///
    /// All batches must have the same schema. The selected column must be a
    /// non-null `FixedSizeList<Float32>` with a positive dimension. Validation
    /// completes before index construction starts.
    pub fn try_new_batches(
        batches: Vec<RecordBatch>,
        vector_column: impl Into<String>,
        metric: Metric,
        index: IndexConfig,
    ) -> DataFusionResult<Self> {
        let vector_column = vector_column.into();
        let Some(first_batch) = batches.first() else {
            return Err(DataFusionError::Plan(
                "a vector table requires at least one record batch".into(),
            ));
        };
        let schema = first_batch.schema();
        for (batch_idx, batch) in batches.iter().enumerate().skip(1) {
            if batch.schema_ref().as_ref() != schema.as_ref() {
                return Err(DataFusionError::Plan(format!(
                    "record batch {batch_idx} does not match the vector table schema"
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
        let mut row_ids = Vec::with_capacity(row_count);
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
                row_ids.push(RowId {
                    batch: batch_idx,
                    row,
                });
                vector_ordinal += 1;
            }
        }

        let dataset = Dataset::try_new(vectors).map_err(core_error)?;
        let index = index.build(dataset, metric).map_err(core_error)?;

        Ok(Self {
            snapshot: Arc::new(VectorTableSnapshot {
                schema,
                batches: batches.into(),
                row_ids: row_ids.into(),
                vector_column: vector_column.into(),
                index,
            }),
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

#[async_trait]
impl TableProvider for VectorTable {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.snapshot.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        Ok(Arc::new(VectorScanExec::try_new(
            Arc::clone(&self.snapshot),
            projection.cloned(),
            limit,
            ScanMode::Full,
            state
                .config_options()
                .extensions
                .get::<VectorSearchOptions>()
                .is_some_and(|options| options.ordered),
            None,
        )?))
    }
}

#[derive(Debug, Clone)]
enum ScanMode {
    Full,
    Vector { query: Arc<[f32]> },
}

#[derive(Debug, Clone)]
struct VectorScanExec {
    snapshot: Arc<VectorTableSnapshot>,
    projection: Option<Vec<usize>>,
    projected_schema: SchemaRef,
    fetch: Option<usize>,
    mode: ScanMode,
    ordered: bool,
    ordering: Option<Vec<PhysicalSortExpr>>,
    properties: Arc<PlanProperties>,
}

impl VectorScanExec {
    fn try_new(
        snapshot: Arc<VectorTableSnapshot>,
        projection: Option<Vec<usize>>,
        fetch: Option<usize>,
        mode: ScanMode,
        ordered: bool,
        ordering: Option<Vec<PhysicalSortExpr>>,
    ) -> DataFusionResult<Self> {
        let projected_schema = project_schema(&snapshot.schema, projection.as_ref())?;
        let properties = compute_properties(&projected_schema, ordering.as_deref());
        Ok(Self {
            snapshot,
            projection,
            projected_schema,
            fetch,
            mode,
            ordered,
            ordering,
            properties: Arc::new(properties),
        })
    }

    fn with_mode_and_ordering(
        &self,
        mode: ScanMode,
        ordering: Vec<PhysicalSortExpr>,
    ) -> DataFusionResult<Self> {
        Self::try_new(
            Arc::clone(&self.snapshot),
            self.projection.clone(),
            self.fetch,
            mode,
            self.ordered,
            Some(ordering),
        )
    }

    fn selected_rows(&self) -> DataFusionResult<Vec<RowId>> {
        let row_count = self.snapshot.row_ids.len();
        match &self.mode {
            ScanMode::Full => Ok(self
                .snapshot
                .row_ids
                .iter()
                .copied()
                .take(self.fetch.unwrap_or(row_count).min(row_count))
                .collect()),
            ScanMode::Vector { query } => {
                let k = self.fetch.unwrap_or(row_count).min(row_count);
                let neighbors = if self.fetch.is_some() {
                    self.snapshot.index.search(query, k)
                } else {
                    FlatIndex::try_new(
                        self.snapshot.index.dataset().clone(),
                        self.snapshot.index.metric(),
                    )
                    .map_err(core_error)?
                    .search(query, k)
                }
                .map_err(core_error)?;
                neighbors
                    .into_iter()
                    .map(|neighbor| {
                        self.snapshot
                            .row_ids
                            .get(neighbor.row)
                            .copied()
                            .ok_or_else(|| {
                                DataFusionError::Internal(format!(
                                    "vector index returned unknown row {}",
                                    neighbor.row
                                ))
                            })
                    })
                    .collect()
            }
        }
    }

    fn output_batch(&self, rows: &[RowId]) -> DataFusionResult<RecordBatch> {
        let projected_columns = self
            .projection
            .clone()
            .unwrap_or_else(|| (0..self.snapshot.schema.fields().len()).collect());
        let fragments = rows
            .iter()
            .map(|row_id| {
                let batch = self.snapshot.batches.get(row_id.batch).ok_or_else(|| {
                    DataFusionError::Internal(format!(
                        "row locator references unknown batch {}",
                        row_id.batch
                    ))
                })?;
                if row_id.row >= batch.num_rows() {
                    return Err(DataFusionError::Internal(format!(
                        "row locator references row {} in a {}-row batch",
                        row_id.row,
                        batch.num_rows()
                    )));
                }
                let columns = projected_columns
                    .iter()
                    .map(|column| batch.column(*column).slice(row_id.row, 1))
                    .collect::<Vec<ArrayRef>>();
                RecordBatch::try_new_with_options(
                    Arc::clone(&self.projected_schema),
                    columns,
                    &RecordBatchOptions::new().with_row_count(Some(1)),
                )
                .map_err(DataFusionError::from)
            })
            .collect::<DataFusionResult<Vec<_>>>()?;
        if fragments.is_empty() {
            return Ok(RecordBatch::new_empty(Arc::clone(&self.projected_schema)));
        }
        concat_batches(&self.projected_schema, &fragments).map_err(DataFusionError::from)
    }
}

impl DisplayAs for VectorScanExec {
    fn fmt_as(&self, format: DisplayFormatType, f: &mut Formatter<'_>) -> fmt::Result {
        match format {
            DisplayFormatType::Default | DisplayFormatType::Verbose => match &self.mode {
                ScanMode::Full => write!(
                    f,
                    "VectorScanExec: rows={}, fetch={:?}",
                    self.snapshot.row_ids.len(),
                    self.fetch
                ),
                ScanMode::Vector { .. } if self.fetch.is_none() => {
                    write!(
                        f,
                        "VectorScanExec: rows={}, fetch=None",
                        self.snapshot.row_ids.len()
                    )
                }
                ScanMode::Vector { query } => write!(
                    f,
                    "VectorIndexScanExec: index={}, metric={:?}, query_dim={}, fetch={:?}, ordered={}",
                    self.snapshot.index.kind(),
                    self.snapshot.index.metric(),
                    query.len(),
                    self.fetch,
                    self.ordered
                ),
            },
            DisplayFormatType::TreeRender => write!(f, "{}", self.name()),
        }
    }
}

impl ExecutionPlan for VectorScanExec {
    fn name(&self) -> &str {
        match (&self.mode, self.fetch) {
            (ScanMode::Full, _) => "VectorScanExec",
            (ScanMode::Vector { .. }, None) => "VectorScanExec",
            (ScanMode::Vector { .. }, Some(_)) => "VectorIndexScanExec",
        }
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
                "VectorScanExec is a leaf plan".into(),
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
                "VectorScanExec has one partition, requested {partition}"
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
        let mut new_scan = self.clone();
        new_scan.fetch = fetch;
        if new_scan.ordered {
            return Some(Arc::new(new_scan));
        }
        let Some(ordering) = new_scan.ordering.clone() else {
            return Some(Arc::new(new_scan));
        };
        let Some(fetch) = fetch else {
            return Some(Arc::new(new_scan));
        };

        // The index selects the candidates; SortExec still owns SQL's ORDER BY contract.
        new_scan.properties = Arc::new(compute_properties(&new_scan.projected_schema, None));
        let ordering = LexOrdering::new(ordering)?;
        let sort = SortExec::new(ordering, Arc::new(new_scan)).with_fetch(Some(fetch));
        Some(Arc::new(sort))
    }

    fn fetch(&self) -> Option<usize> {
        self.fetch
    }

    fn try_pushdown_sort(
        &self,
        order: &[PhysicalSortExpr],
    ) -> DataFusionResult<SortOrderPushdownResult<Arc<dyn ExecutionPlan>>> {
        let Some(query) = match_vector_order(
            order,
            &self.projected_schema,
            self.snapshot.index.as_ref(),
            &self.snapshot.vector_column,
        ) else {
            return Ok(SortOrderPushdownResult::Unsupported);
        };
        let scan = self.with_mode_and_ordering(
            ScanMode::Vector {
                query: query.into(),
            },
            order.to_vec(),
        )?;
        Ok(SortOrderPushdownResult::Exact {
            inner: Arc::new(scan),
        })
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
    let function = uncast(sort.expr.as_ref()).downcast_ref::<ScalarFunctionExpr>()?;
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
        uncast(left.as_ref()).downcast_ref::<Column>(),
        uncast(right.as_ref()).downcast_ref::<Literal>(),
    ) {
        (Some(column), Some(literal)) => (column, literal),
        _ => match (
            uncast(right.as_ref()).downcast_ref::<Column>(),
            uncast(left.as_ref()).downcast_ref::<Literal>(),
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

fn metric_for_function(name: &str) -> Option<Metric> {
    match name {
        "array_distance" | "list_distance" => Some(Metric::Euclidean),
        "cosine_distance" => Some(Metric::Cosine),
        "inner_product" | "dot_product" => Some(Metric::Dot),
        _ => None,
    }
}

fn uncast(expression: &dyn PhysicalExpr) -> &dyn PhysicalExpr {
    if let Some(cast) = expression.downcast_ref::<CastExpr>() {
        uncast(cast.expr.as_ref())
    } else {
        expression
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
        return Some(values.values().iter().map(|value| *value as f32).collect());
    }
    if let Some(values) = values.as_any().downcast_ref::<Int32Array>() {
        return Some(values.values().iter().map(|value| *value as f32).collect());
    }
    if let Some(values) = values.as_any().downcast_ref::<Int64Array>() {
        return Some(values.values().iter().map(|value| *value as f32).collect());
    }
    None
}

fn core_error(error: vector_core::VectorError) -> DataFusionError {
    DataFusionError::Plan(error.to_string())
}
