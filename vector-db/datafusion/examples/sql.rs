use std::io::{self, BufRead};

use datafusion::arrow::util::pretty::print_batches;
use datafusion::common::{DataFusionError, Result};
use vector_core::{IndexConfig, IvfFlatConfig, Metric};
use vector_datafusion::{VectorSqlOutput, VectorSqlSession};

#[tokio::main]
async fn main() -> Result<()> {
    let mut session = VectorSqlSession::new(
        Metric::Cosine,
        IndexConfig::IvfFlat(IvfFlatConfig {
            partitions: 2,
            probes: 2,
            iterations: 8,
            seed: 7,
        }),
    );
    println!("Enter one SQL statement per line.");
    for line in io::stdin().lock().lines() {
        let sql = line.map_err(DataFusionError::IoError)?;
        if sql.trim().is_empty() {
            continue;
        }
        match session.execute(&sql).await? {
            VectorSqlOutput::Query(batches) => print_batches(&batches)?,
            VectorSqlOutput::StatementComplete(rows) => println!("{rows} rows affected"),
            VectorSqlOutput::CreatedIndex(name) => println!("created vector index {name}"),
        }
    }
    Ok(())
}
