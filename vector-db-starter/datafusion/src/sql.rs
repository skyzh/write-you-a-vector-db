use std::any::Any;
use std::collections::HashSet;
use std::sync::Arc;

use datafusion::arrow::array::{Array, UInt64Array};
use datafusion::arrow::datatypes::DataType;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::{
    DataFusionError, ResolvedTableReference, Result as DataFusionResult, TableReference,
};
use datafusion::datasource::{MemTable, TableProvider};
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use datafusion::sql::parser::{DFParser, Statement};
use datafusion::sql::sqlparser::ast::{
    CreateIndex, Expr, Ident, IndexType, ObjectName, ObjectNamePart, ObjectType,
    Statement as SqlStatement, TableObject,
};
use vector_core::{IndexConfig, Metric};

use crate::{VectorIndexAttachment, with_vector_indexes, with_vector_search_options};

/// The result of executing one statement through [`VectorSqlSession`].
#[derive(Debug)]
pub enum VectorSqlOutput {
    Query(Vec<RecordBatch>),
    StatementComplete(u64),
    CreatedIndex(String),
}

#[derive(Debug, Clone)]
struct RegisteredIndex {
    name: String,
    table: ResolvedTableReference,
    column: String,
    attachment: VectorIndexAttachment,
}

/// A small SQL session that adds course vector-index DDL to DataFusion.
///
/// Ordinary SQL is still planned and executed by DataFusion. This adapter owns
/// only the bounded `CREATE INDEX ... USING <kind> (column)` bridge and the
/// attachment state needed by the vector physical optimizer.
pub struct VectorSqlSession {
    base: SessionContext,
    context: SessionContext,
    metric: Metric,
    index: IndexConfig,
    indexes: Vec<RegisteredIndex>,
}

impl VectorSqlSession {
    pub fn new(metric: Metric, index: IndexConfig) -> Self {
        let base =
            SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
        Self {
            context: base.clone(),
            base,
            metric,
            index,
            indexes: Vec::new(),
        }
    }

    /// Execute exactly one SQL statement.
    pub async fn execute(&mut self, sql: &str) -> DataFusionResult<VectorSqlOutput> {
        let mut statements = DFParser::parse_sql(sql)?;
        if statements.len() != 1 {
            return Err(DataFusionError::Plan(
                "the vector SQL session accepts exactly one statement at a time".into(),
            ));
        }
        let statement = statements.pop_front().expect("one statement was checked");
        let table_refs = self.base.state().resolve_table_references(&statement)?;

        match statement {
            Statement::Statement(statement) => match *statement {
                SqlStatement::CreateIndex(create) => self.create_index(create, table_refs).await,
                statement if is_query(&statement) => self.query(sql).await,
                statement => {
                    self.reject_stale_index_mutation(&statement, table_refs)?;
                    self.statement(sql).await
                }
            },
            _ => self.query(sql).await,
        }
    }

    async fn query(&self, sql: &str) -> DataFusionResult<VectorSqlOutput> {
        Ok(VectorSqlOutput::Query(
            self.context.sql(sql).await?.collect().await?,
        ))
    }

    async fn statement(&mut self, sql: &str) -> DataFusionResult<VectorSqlOutput> {
        let batches = self.base.sql(sql).await?.collect().await?;
        let affected = statement_count(&batches)?;
        self.rebuild_context();
        Ok(VectorSqlOutput::StatementComplete(affected))
    }

    async fn create_index(
        &mut self,
        create: CreateIndex,
        table_refs: Vec<TableReference>,
    ) -> DataFusionResult<VectorSqlOutput> {
        validate_plain_create_index(&create)?;
        let name = create
            .name
            .as_ref()
            .map(|name| normalize_object_name(name, self.ident_normalization()))
            .ok_or_else(|| DataFusionError::Plan("CREATE INDEX requires an index name".into()))?;
        if self.indexes.iter().any(|index| index.name == name) {
            return Err(DataFusionError::Plan(format!(
                "vector index name '{name}' already exists"
            )));
        }

        let expected_kind = index_kind(&self.index)?;
        let actual_kind = match &create.using {
            Some(IndexType::Custom(kind)) => normalize_ident(kind, self.ident_normalization()),
            _ => {
                return Err(DataFusionError::Plan(format!(
                    "CREATE INDEX requires USING {expected_kind}"
                )));
            }
        };
        if actual_kind != expected_kind {
            return Err(DataFusionError::Plan(format!(
                "this vector SQL session requires USING {expected_kind}, got {actual_kind}"
            )));
        }

        if table_refs.len() != 1 {
            return Err(DataFusionError::Plan(
                "CREATE INDEX must resolve exactly one target table".into(),
            ));
        }
        let table = self.resolve_table(table_refs[0].clone());
        let column = validate_index_column(&create, self.ident_normalization())?;
        if self
            .indexes
            .iter()
            .any(|index| index.table == table && index.column == column)
        {
            return Err(DataFusionError::Plan(format!(
                "vector index target '{table}.{column}' already exists"
            )));
        }

        let provider = self.base.table_provider(table.clone()).await?;
        let schema = provider.schema();
        let field = schema.field_with_name(&column).map_err(|_| {
            DataFusionError::Plan(format!(
                "vector column '{column}' does not exist in table '{table}'"
            ))
        })?;
        if field.is_nullable() {
            return Err(DataFusionError::Plan(format!(
                "vector column '{column}' in table '{table}' must be NOT NULL"
            )));
        }
        let DataType::FixedSizeList(item, dimension) = field.data_type() else {
            return Err(DataFusionError::Plan(format!(
                "vector column '{column}' must be FixedSizeList<Float32>, got {}",
                field.data_type()
            )));
        };
        if item.data_type() != &DataType::Float32 {
            return Err(DataFusionError::Plan(format!(
                "vector column '{column}' must be FixedSizeList<Float32>, got {}",
                field.data_type()
            )));
        }
        if *dimension <= 0 {
            return Err(DataFusionError::Plan(format!(
                "vector column '{column}' dimension must be greater than zero"
            )));
        }
        let mem_table = downcast_mem_table(provider, &table)?;
        let attachment = VectorIndexAttachment::try_new(
            &self.base,
            table.clone(),
            &mem_table,
            column.clone(),
            self.metric,
            self.index.clone(),
        )
        .await?;

        self.indexes.push(RegisteredIndex {
            name: name.clone(),
            table,
            column,
            attachment,
        });
        self.rebuild_context();
        Ok(VectorSqlOutput::CreatedIndex(name))
    }

    fn reject_stale_index_mutation(
        &self,
        statement: &SqlStatement,
        mut table_refs: Vec<TableReference>,
    ) -> DataFusionResult<()> {
        if !is_mutation(statement) {
            return Ok(());
        }
        match statement {
            SqlStatement::Insert(insert) => {
                if let TableObject::TableName(name) = &insert.table {
                    table_refs = vec![object_name_table_reference(
                        name,
                        self.ident_normalization(),
                    )?];
                }
            }
            SqlStatement::AlterTable(alter) => {
                table_refs = vec![object_name_table_reference(
                    &alter.name,
                    self.ident_normalization(),
                )?];
            }
            SqlStatement::Drop {
                object_type: ObjectType::Table,
                names,
                ..
            } => {
                table_refs = names
                    .iter()
                    .map(|name| object_name_table_reference(name, self.ident_normalization()))
                    .collect::<DataFusionResult<_>>()?;
            }
            _ => {}
        }
        let indexed_tables = self
            .indexes
            .iter()
            .map(|index| &index.table)
            .collect::<HashSet<_>>();
        for table_ref in table_refs {
            let table = self.resolve_table(table_ref);
            if indexed_tables.contains(&table) {
                return Err(DataFusionError::Plan(format!(
                    "mutation of indexed table '{table}' would make its vector index stale"
                )));
            }
        }
        Ok(())
    }

    fn rebuild_context(&mut self) {
        self.context = with_vector_indexes(
            &self.base,
            self.indexes
                .iter()
                .map(|index| index.attachment.clone())
                .collect(),
        );
    }

    fn ident_normalization(&self) -> bool {
        self.base
            .state()
            .config_options()
            .sql_parser
            .enable_ident_normalization
    }

    fn resolve_table(&self, table: TableReference) -> ResolvedTableReference {
        let state = self.base.state();
        let catalog = &state.config_options().catalog;
        table.resolve(&catalog.default_catalog, &catalog.default_schema)
    }
}

fn validate_plain_create_index(create: &CreateIndex) -> DataFusionResult<()> {
    if create.unique
        || create.concurrently
        || create.if_not_exists
        || !create.include.is_empty()
        || create.nulls_distinct.is_some()
        || !create.with.is_empty()
        || create.predicate.is_some()
        || !create.index_options.is_empty()
        || !create.alter_options.is_empty()
    {
        return Err(DataFusionError::Plan(
            "the vector SQL session supports only plain CREATE INDEX ... USING <kind> (column)"
                .into(),
        ));
    }
    Ok(())
}

fn validate_index_column(create: &CreateIndex, normalize: bool) -> DataFusionResult<String> {
    if create.columns.len() != 1
        || create.columns[0].operator_class.is_some()
        || create.columns[0].column.options.asc.is_some()
        || create.columns[0].column.options.nulls_first.is_some()
        || create.columns[0].column.with_fill.is_some()
    {
        return Err(DataFusionError::Plan(
            "a vector index requires exactly one plain vector column".into(),
        ));
    }
    let Expr::Identifier(column) = &create.columns[0].column.expr else {
        return Err(DataFusionError::Plan(
            "a vector index requires exactly one plain vector column".into(),
        ));
    };
    Ok(normalize_ident(column, normalize))
}

fn normalize_ident(ident: &Ident, normalize: bool) -> String {
    if normalize && ident.quote_style.is_none() {
        ident.value.to_ascii_lowercase()
    } else {
        ident.value.clone()
    }
}

fn normalize_object_name(name: &ObjectName, normalize: bool) -> String {
    name.0
        .iter()
        .map(|part| match part {
            ObjectNamePart::Identifier(ident) => normalize_ident(ident, normalize),
            ObjectNamePart::Function(function) => function.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn object_name_table_reference(
    name: &ObjectName,
    normalize: bool,
) -> DataFusionResult<TableReference> {
    let parts = name
        .0
        .iter()
        .map(|part| match part {
            ObjectNamePart::Identifier(ident) => Ok(normalize_ident(ident, normalize)),
            ObjectNamePart::Function(_) => Err(DataFusionError::Plan(
                "table names cannot contain function parts".into(),
            )),
        })
        .collect::<DataFusionResult<Vec<_>>>()?;
    match parts.as_slice() {
        [table] => Ok(TableReference::bare(table.as_str())),
        [schema, table] => Ok(TableReference::partial(schema.as_str(), table.as_str())),
        [catalog, schema, table] => Ok(TableReference::full(
            catalog.as_str(),
            schema.as_str(),
            table.as_str(),
        )),
        _ => Err(DataFusionError::Plan(
            "table names may contain at most catalog, schema, and table parts".into(),
        )),
    }
}

fn index_kind(index: &IndexConfig) -> DataFusionResult<&'static str> {
    match index {
        IndexConfig::Flat => Ok("flat"),
        IndexConfig::IvfFlat(_) => Ok("ivfflat"),
        IndexConfig::Nsw(_) => Ok("nsw"),
        IndexConfig::Hnsw(_) => Ok("hnsw"),
        IndexConfig::IvfPq(_) => Ok("ivfpq"),
    }
}

fn downcast_mem_table(
    provider: Arc<dyn TableProvider>,
    table: &ResolvedTableReference,
) -> DataFusionResult<Arc<MemTable>> {
    let provider: Arc<dyn Any + Send + Sync> = provider;
    Arc::downcast::<MemTable>(provider).map_err(|_| {
        DataFusionError::Plan(format!(
            "vector index target '{table}' must be a DataFusion MemTable"
        ))
    })
}

fn statement_count(batches: &[RecordBatch]) -> DataFusionResult<u64> {
    let mut count = 0;
    for batch in batches {
        if batch.num_columns() == 0 {
            continue;
        }
        let Some(values) = batch.column(0).as_any().downcast_ref::<UInt64Array>() else {
            return Err(DataFusionError::Execution(
                "DataFusion statement output did not contain a UInt64 affected-row count".into(),
            ));
        };
        for row in 0..values.len() {
            if !values.is_null(row) {
                count += values.value(row);
            }
        }
    }
    Ok(count)
}

fn is_query(statement: &SqlStatement) -> bool {
    matches!(
        statement,
        SqlStatement::Query(_)
            | SqlStatement::Explain { .. }
            | SqlStatement::ExplainTable { .. }
            | SqlStatement::ShowFunctions { .. }
            | SqlStatement::ShowVariable { .. }
            | SqlStatement::ShowStatus { .. }
            | SqlStatement::ShowVariables { .. }
            | SqlStatement::ShowCreate { .. }
            | SqlStatement::ShowColumns { .. }
            | SqlStatement::ShowCatalogs { .. }
            | SqlStatement::ShowDatabases { .. }
            | SqlStatement::ShowProcessList { .. }
            | SqlStatement::ShowSchemas { .. }
            | SqlStatement::ShowCharset(_)
            | SqlStatement::ShowObjects(_)
            | SqlStatement::ShowTables { .. }
            | SqlStatement::ShowViews { .. }
            | SqlStatement::ShowCollation { .. }
    )
}

fn is_mutation(statement: &SqlStatement) -> bool {
    matches!(
        statement,
        SqlStatement::Insert(_)
            | SqlStatement::Update(_)
            | SqlStatement::Delete(_)
            | SqlStatement::Merge(_)
            | SqlStatement::Truncate(_)
            | SqlStatement::AlterTable(_)
            | SqlStatement::Drop {
                object_type: ObjectType::Table,
                ..
            }
    )
}

#[cfg(test)]
mod tests {
    use datafusion::arrow::array::Int64Array;
    use datafusion::arrow::datatypes::{Field, Schema};
    use datafusion::arrow::util::display::array_value_to_string;
    use datafusion::datasource::empty::EmptyTable;
    use vector_core::IvfFlatConfig;

    use super::*;

    const QUERY: &str = "SELECT id, payload FROM points \
        ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3";

    fn session() -> VectorSqlSession {
        VectorSqlSession::new(
            Metric::Cosine,
            IndexConfig::IvfFlat(IvfFlatConfig {
                partitions: 2,
                probes: 2,
                iterations: 8,
                seed: 7,
            }),
        )
    }

    async fn statement(session: &mut VectorSqlSession, sql: &str) -> u64 {
        let VectorSqlOutput::StatementComplete(count) = session.execute(sql).await.unwrap() else {
            panic!("expected statement output")
        };
        count
    }

    async fn query(session: &mut VectorSqlSession, sql: &str) -> Vec<RecordBatch> {
        let VectorSqlOutput::Query(batches) = session.execute(sql).await.unwrap() else {
            panic!("expected query output")
        };
        batches
    }

    async fn create_points(session: &mut VectorSqlSession) {
        assert_eq!(
            statement(
                session,
                "CREATE TABLE points (id BIGINT NOT NULL, payload VARCHAR NOT NULL, \
                 embedding REAL[3] NOT NULL)"
            )
            .await,
            0
        );
        assert_eq!(
            statement(
                session,
                "INSERT INTO points VALUES \
                 (1, 'one', [1.0, 0.0, 0.0]), \
                 (2, 'two', [0.9, 0.1, 0.0]), \
                 (3, 'three', [0.0, 1.0, 0.0]), \
                 (4, 'four', [-1.0, 0.0, 0.0])"
            )
            .await,
            4
        );
    }

    fn rows(batches: &[RecordBatch]) -> Vec<(i64, String)> {
        batches
            .iter()
            .flat_map(|batch| {
                let ids = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .unwrap();
                (0..batch.num_rows())
                    .map(|row| {
                        (
                            ids.value(row),
                            array_value_to_string(batch.column(1).as_ref(), row).unwrap(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn plan_text(batches: &[RecordBatch]) -> String {
        batches.iter().map(|batch| format!("{batch:?}")).collect()
    }

    #[tokio::test]
    async fn empty_session_forwards_sql_and_attaches_the_configured_index() {
        let mut session = session();
        create_points(&mut session).await;
        let before = query(&mut session, QUERY).await;
        let before_plan = plan_text(&query(&mut session, &format!("EXPLAIN {QUERY}")).await);
        assert!(before_plan.contains("DataSourceExec"), "{before_plan}");

        let created = session
            .execute("CREATE INDEX arbitrary_name ON points USING ivfflat (embedding)")
            .await
            .unwrap();
        assert!(matches!(
            created,
            VectorSqlOutput::CreatedIndex(ref name) if name == "arbitrary_name"
        ));
        let after = query(&mut session, QUERY).await;
        let after_plan = plan_text(&query(&mut session, &format!("EXPLAIN {QUERY}")).await);
        assert!(after_plan.contains("VectorIndexScanExec"), "{after_plan}");
        assert!(after_plan.contains("index=ivf_flat"), "{after_plan}");
        assert_eq!(rows(&before), rows(&after));
        assert_eq!(
            rows(&after),
            [(1, "one".into()), (2, "two".into()), (3, "three".into())]
        );
    }

    #[tokio::test]
    async fn validation_does_not_consume_names_and_rejects_bad_targets() {
        let mut session = session();
        statement(
            &mut session,
            "CREATE TABLE wrong (id BIGINT NOT NULL, payload VARCHAR NOT NULL)",
        )
        .await;
        statement(
            &mut session,
            "CREATE TABLE nullable_vectors (id BIGINT NOT NULL, embedding REAL[3])",
        )
        .await;
        create_points(&mut session).await;

        for (sql, message) in [
            (
                "CREATE INDEX reusable ON missing USING ivfflat (embedding)",
                "No table named",
            ),
            (
                "CREATE INDEX reusable ON wrong USING ivfflat (payload)",
                "FixedSizeList<Float32>",
            ),
            (
                "CREATE INDEX reusable ON nullable_vectors USING ivfflat (embedding)",
                "must be NOT NULL",
            ),
            (
                "CREATE INDEX reusable ON points USING flat (embedding)",
                "requires USING ivfflat",
            ),
            (
                "CREATE INDEX reusable ON points USING ivfflat (missing)",
                "does not exist",
            ),
            (
                "CREATE INDEX reusable ON points USING ivfflat (embedding, payload)",
                "exactly one plain vector column",
            ),
        ] {
            let error = session.execute(sql).await.unwrap_err().to_string();
            assert!(error.contains(message), "{sql}: {error}");
        }
        assert!(matches!(
            session
                .execute("CREATE INDEX reusable ON points USING ivfflat (embedding)")
                .await
                .unwrap(),
            VectorSqlOutput::CreatedIndex(ref name) if name == "reusable"
        ));
    }

    #[tokio::test]
    async fn qualified_targets_multiple_attachments_and_duplicates_are_explicit() {
        let mut session = session();
        create_points(&mut session).await;
        statement(&mut session, "CREATE SCHEMA other").await;
        statement(
            &mut session,
            "CREATE TABLE other.points (id BIGINT NOT NULL, embedding REAL[3] NOT NULL)",
        )
        .await;
        statement(
            &mut session,
            "INSERT INTO other.points VALUES \
             (9, [1.0, 0.0, 0.0]), \
             (10, [0.0, 1.0, 0.0])",
        )
        .await;

        session
            .execute("CREATE INDEX first_idx ON public.points USING ivfflat (embedding)")
            .await
            .unwrap();
        session
            .execute("CREATE INDEX second_idx ON other.points USING ivfflat (embedding)")
            .await
            .unwrap();

        let duplicate_name = session
            .execute("CREATE INDEX first_idx ON other.points USING ivfflat (embedding)")
            .await
            .unwrap_err()
            .to_string();
        assert!(duplicate_name.contains("name 'first_idx' already exists"));
        let duplicate_target = session
            .execute("CREATE INDEX third_idx ON public.points USING ivfflat (embedding)")
            .await
            .unwrap_err()
            .to_string();
        assert!(duplicate_target.contains("target"));
    }

    #[tokio::test]
    async fn indexed_mutation_is_rejected_but_unindexed_mutation_remains_legal() {
        let mut session = session();
        create_points(&mut session).await;
        statement(
            &mut session,
            "CREATE TABLE audit_log (id BIGINT NOT NULL, payload VARCHAR NOT NULL)",
        )
        .await;
        session
            .execute("CREATE INDEX points_idx ON points USING ivfflat (embedding)")
            .await
            .unwrap();

        let stale = session
            .execute("INSERT INTO points VALUES (5, 'five', [0.0, 0.0, 1.0])")
            .await
            .unwrap_err()
            .to_string();
        assert!(
            stale.contains("would make its vector index stale"),
            "{stale}"
        );
        let drop = session
            .execute("DROP TABLE points")
            .await
            .unwrap_err()
            .to_string();
        assert!(drop.contains("would make its vector index stale"), "{drop}");
        assert_eq!(
            statement(&mut session, "INSERT INTO audit_log VALUES (1, 'ok')").await,
            1
        );
        assert_eq!(
            statement(
                &mut session,
                "INSERT INTO audit_log SELECT id, payload FROM points WHERE id = 2"
            )
            .await,
            1
        );
        assert_eq!(rows(&query(&mut session, QUERY).await).len(), 3);
    }

    #[tokio::test]
    async fn non_memtable_and_zero_width_are_rejected_before_name_registration() {
        let mut session = session();
        let non_memtable = Arc::new(EmptyTable::new(Arc::new(Schema::new(vec![Field::new(
            "embedding",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 3),
            false,
        )]))));
        session
            .base
            .register_table("external", non_memtable)
            .unwrap();
        let error = session
            .execute("CREATE INDEX reusable ON external USING ivfflat (embedding)")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("must be a DataFusion MemTable"), "{error}");

        let zero_schema = Arc::new(Schema::new(vec![Field::new(
            "embedding",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 0),
            false,
        )]));
        let zero = Arc::new(MemTable::try_new(zero_schema, vec![Vec::new()]).unwrap());
        session.base.register_table("zero_width", zero).unwrap();
        let error = session
            .execute("CREATE INDEX reusable ON zero_width USING ivfflat (embedding)")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("greater than zero"), "{error}");
    }

    #[test]
    fn adapters_delegate_to_the_shared_executor() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/sql.rs"));
        let production = source.split("\n#[cfg(test)]").next().unwrap();
        let example = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/sql.rs"));
        let runner = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/sqllogictest.rs"
        ));
        if env!("CARGO_PKG_NAME") == "vector-datafusion" {
            assert!(example.contains("session.execute(&sql)"));
            assert!(!example.contains("DFParser"));
        } else {
            assert!(!example.contains("VectorSqlSession"));
        }
        assert!(runner.contains("self.session.execute(sql)"));
        assert!(!runner.contains("register_table"));
        assert!(!runner.contains("VectorIndexAttachment"));
        assert!(production.contains("downcast_mem_table(provider, &table)?"));
        assert!(production.contains("let DataType::FixedSizeList(item, dimension)"));
        assert!(production.contains("if item.data_type() != &DataType::Float32 {"));
        assert!(production.contains("if *dimension <= 0 {"));
    }
}
