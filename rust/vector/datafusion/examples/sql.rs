use std::io::{self, BufRead};
use std::sync::Arc;

use datafusion::arrow::record_batch::RecordBatch;
use datafusion::arrow::util::pretty::print_batches;
use datafusion::common::{DataFusionError, Result};
use datafusion::datasource::MemTable;
use datafusion::execution::config::SessionConfig;
use datafusion::execution::context::SessionContext;
use datafusion::sql::parser::{DFParser, Statement};
use datafusion::sql::sqlparser::ast::{CreateIndex, Expr, IndexType, Statement as SqlStatement};
use vector_core::{IndexConfig, IvfFlatConfig, Metric};
use vector_datafusion::{
    VectorIndexAttachment, VectorRow, vector_mem_table, with_vector_indexes,
    with_vector_search_options,
};

#[cfg(test)]
const QUERY: &str = "SELECT id, payload FROM points \
    ORDER BY cosine_distance(embedding, [1.0, 0.0, 0.0]) LIMIT 3";
const INDEX_NAME: &str = "points_embedding_idx";

#[derive(Debug)]
enum ShellOutput {
    Query(Vec<RecordBatch>),
    CreatedIndex(String),
}

struct VectorSqlShell {
    base: SessionContext,
    context: SessionContext,
    points: Arc<MemTable>,
    has_index: bool,
}

impl VectorSqlShell {
    fn new() -> Result<Self> {
        let base =
            SessionContext::new_with_config(with_vector_search_options(SessionConfig::new()));
        let points = vector_mem_table(vec![
            VectorRow::new(1, vec![1.0, 0.0, 0.0], "one"),
            VectorRow::new(2, vec![0.9, 0.1, 0.0], "two"),
            VectorRow::new(3, vec![0.0, 1.0, 0.0], "three"),
            VectorRow::new(4, vec![-1.0, 0.0, 0.0], "four"),
            VectorRow::new(5, vec![0.0, 0.0, 1.0], "five"),
        ])?;
        base.register_table("points", points.clone())?;
        let context = base.clone();
        Ok(Self {
            base,
            context,
            points,
            has_index: false,
        })
    }

    async fn execute(&mut self, sql: &str) -> Result<ShellOutput> {
        let mut statements = DFParser::parse_sql(sql)?;
        if statements.len() != 1 {
            return Err(DataFusionError::Plan(
                "the vector shell accepts exactly one SQL statement at a time".into(),
            ));
        }
        let statement = statements.pop_front().expect("one statement was checked");
        let Statement::Statement(statement) = statement else {
            return self.query(sql).await;
        };
        let SqlStatement::CreateIndex(create) = *statement else {
            return self.query(sql).await;
        };
        let name = validate_create_index(&create)?;
        if self.has_index {
            return Err(DataFusionError::Plan(
                "the course shell supports one vector index per session".into(),
            ));
        }

        let attachment = VectorIndexAttachment::try_new(
            &self.base,
            "points",
            &self.points,
            "embedding",
            Metric::Cosine,
            IndexConfig::IvfFlat(IvfFlatConfig {
                partitions: 2,
                probes: 2,
                iterations: 8,
                seed: 7,
            }),
        )
        .await?;
        self.context = with_vector_indexes(&self.base, vec![attachment]);
        self.has_index = true;
        Ok(ShellOutput::CreatedIndex(name))
    }

    async fn query(&self, sql: &str) -> Result<ShellOutput> {
        Ok(ShellOutput::Query(
            self.context.sql(sql).await?.collect().await?,
        ))
    }
}

fn validate_create_index(create: &CreateIndex) -> Result<String> {
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
            "the course shell supports only plain CREATE INDEX ... USING ivfflat (column)".into(),
        ));
    }
    let name = create
        .name
        .as_ref()
        .ok_or_else(|| DataFusionError::Plan("CREATE INDEX requires an index name".into()))?
        .to_string();
    if !name.eq_ignore_ascii_case(INDEX_NAME) {
        return Err(DataFusionError::Plan(format!(
            "the course shell requires the index name {INDEX_NAME}"
        )));
    }
    if !create.table_name.to_string().eq_ignore_ascii_case("points") {
        return Err(DataFusionError::Plan(
            "CREATE INDEX in the course shell targets only the loaded points table".into(),
        ));
    }
    match &create.using {
        Some(IndexType::Custom(kind)) if kind.value.eq_ignore_ascii_case("ivfflat") => {}
        _ => {
            return Err(DataFusionError::Plan(
                "CREATE INDEX requires USING ivfflat".into(),
            ));
        }
    }
    if create.columns.len() != 1
        || create.columns[0].operator_class.is_some()
        || !matches!(
            &create.columns[0].column.expr,
            Expr::Identifier(column) if column.value.eq_ignore_ascii_case("embedding")
        )
        || create.columns[0].column.to_string() != create.columns[0].column.expr.to_string()
    {
        return Err(DataFusionError::Plan(
            "the IVFFlat index requires the single vector column embedding".into(),
        ));
    }
    Ok(name)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut shell = VectorSqlShell::new()?;
    println!("Loaded points(id, payload, embedding). Enter one SQL statement per line.");
    for line in io::stdin().lock().lines() {
        let sql = line.map_err(DataFusionError::IoError)?;
        if sql.trim().is_empty() {
            continue;
        }
        match shell.execute(&sql).await? {
            ShellOutput::Query(batches) => print_batches(&batches)?,
            ShellOutput::CreatedIndex(name) => println!("created vector index {name}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use datafusion::arrow::array::{StringArray, UInt64Array};

    use super::*;

    fn rows(batches: &[RecordBatch]) -> Vec<(u64, String)> {
        batches
            .iter()
            .flat_map(|batch| {
                let ids = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap();
                let payloads = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                (0..batch.num_rows())
                    .map(|row| (ids.value(row), payloads.value(row).to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn plan_text(batches: &[RecordBatch]) -> String {
        batches.iter().map(|batch| format!("{batch:?}")).collect()
    }

    async fn query(shell: &mut VectorSqlShell, sql: &str) -> Vec<RecordBatch> {
        let ShellOutput::Query(batches) = shell.execute(sql).await.unwrap() else {
            panic!("expected query output")
        };
        batches
    }

    async fn assert_rejected_without_consuming_index(sql: &str, message: &str) {
        let mut shell = VectorSqlShell::new().unwrap();
        let error = shell.execute(sql).await.unwrap_err().to_string();
        assert!(error.contains(message), "{sql}: {error}");
        let output = shell
            .execute("CREATE INDEX points_embedding_idx ON points USING ivfflat (embedding)")
            .await
            .unwrap();
        assert!(matches!(
            output,
            ShellOutput::CreatedIndex(ref name) if name == INDEX_NAME
        ));
    }

    #[tokio::test]
    async fn create_index_execute_accepts_only_the_documented_command() {
        for (sql, message) in [
            (
                "CREATE INDEX other ON points USING ivfflat (embedding)",
                "index name",
            ),
            (
                "CREATE INDEX points_embedding_idx ON other USING ivfflat (embedding)",
                "loaded points table",
            ),
            (
                "CREATE INDEX points_embedding_idx ON points USING ivfflat (payload)",
                "vector column embedding",
            ),
            (
                "CREATE INDEX points_embedding_idx ON points USING hnsw (embedding)",
                "USING ivfflat",
            ),
            (
                "CREATE UNIQUE INDEX points_embedding_idx ON points USING ivfflat (embedding)",
                "only plain CREATE INDEX",
            ),
        ] {
            assert_rejected_without_consuming_index(sql, message).await;
        }
    }

    #[tokio::test]
    async fn create_index_attaches_ivfflat_without_changing_query_results() {
        let mut shell = VectorSqlShell::new().unwrap();
        let before = query(&mut shell, QUERY).await;
        let before_plan = plan_text(&query(&mut shell, &format!("EXPLAIN {QUERY}")).await);
        assert!(before_plan.contains("DataSourceExec"), "{before_plan}");
        assert!(
            !before_plan.contains("VectorIndexScanExec"),
            "{before_plan}"
        );
        let output = shell
            .execute("CREATE INDEX points_embedding_idx ON points USING ivfflat (embedding)")
            .await
            .unwrap();
        assert!(matches!(
            output,
            ShellOutput::CreatedIndex(ref name) if name == INDEX_NAME
        ));

        let after = query(&mut shell, QUERY).await;
        let after_plan = plan_text(&query(&mut shell, &format!("EXPLAIN {QUERY}")).await);
        assert!(after_plan.contains("VectorIndexScanExec"), "{after_plan}");
        assert!(after_plan.contains("index=ivf_flat"), "{after_plan}");
        assert!(!after_plan.contains("DataSourceExec"), "{after_plan}");
        assert_eq!(rows(&before), rows(&after));
        assert_eq!(
            rows(&after),
            [
                (1, "one".to_owned()),
                (2, "two".to_owned()),
                (3, "three".to_owned()),
            ]
        );

        let duplicate = shell
            .execute("CREATE INDEX points_embedding_idx ON points USING ivfflat (embedding)")
            .await
            .unwrap_err()
            .to_string();
        assert!(duplicate.contains("one vector index per session"));

        let multiple = shell
            .execute("SELECT 1; SELECT 2")
            .await
            .unwrap_err()
            .to_string();
        assert!(multiple.contains("exactly one SQL statement"));
    }

    #[tokio::test]
    async fn datafusion_has_no_create_index_physical_executor() {
        let shell = VectorSqlShell::new().unwrap();
        let dataframe = shell
            .context
            .sql("CREATE INDEX points_embedding_idx ON points USING ivfflat (embedding)")
            .await
            .unwrap();
        let error = dataframe.collect().await.unwrap_err().to_string();
        assert!(error.contains("CreateIndex"), "{error}");
    }
}
