use std::fmt::{self, Formatter};
use std::sync::Arc;

use datafusion::arrow::array::{Array, Float32Array, Float64Array, Int32Array, Int64Array};
use datafusion::arrow::datatypes::{DataType, Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::config::ConfigExtension;
use datafusion::common::tree_node::{Transformed, TreeNode};
use datafusion::common::{
    DataFusionError, Result as DataFusionResult, ScalarValue, extensions_options,
};
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::datasource::source::DataSourceExec;
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
use vector_core::{Metric, VectorIndex};

use crate::attachment::{RowId, VectorIndexAttachment, VectorIndexSnapshot};
use crate::core_error;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
