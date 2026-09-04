use std::fs;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::common::{DataFusionError, Result};
use datafusion::dataframe::DataFrame;
use datafusion::execution::{TaskContext, context::SessionState};
use datafusion::logical_expr::LogicalPlan;
use datafusion::object_store::ObjectStore;
use datafusion::prelude::SessionContext;
use datafusion::sql::parser::DFParser;
use datafusion_cli::DATAFUSION_CLI_VERSION;
use datafusion_cli::cli_context::CliSessionContext;
use datafusion_cli::command::{Command, OutputFormat};
use datafusion_cli::exec::exec_from_commands;
use datafusion_cli::object_storage::instrumented::InstrumentedObjectStoreRegistry;
use datafusion_cli::print_format::PrintFormat;
use datafusion_cli::print_options::{MaxRows, PrintOptions};
use tokio::sync::Mutex;
use vector_core::{IndexConfig, IvfFlatConfig, Metric};
use vector_datafusion::VectorSqlSession;

struct VectorCliContext {
    planner: SessionContext,
    session: Mutex<VectorSqlSession>,
}

impl VectorCliContext {
    fn new(session: VectorSqlSession) -> Self {
        Self {
            planner: session.cli_session_context(),
            session: Mutex::new(session),
        }
    }

    async fn execute_sql(&self, sql: String, print_options: &PrintOptions) -> Result<()> {
        self.session.lock().await.validate_cli_sql(&sql)?;
        exec_from_commands(self, vec![sql], print_options).await
    }

    async fn execute_sql_continuing(&self, sql: &str, print_options: &PrintOptions) -> Result<()> {
        for statement in DFParser::parse_sql(sql)? {
            if let Err(error) = self.execute_sql(statement.to_string(), print_options).await {
                eprintln!("{error}");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl CliSessionContext for VectorCliContext {
    fn task_ctx(&self) -> Arc<TaskContext> {
        CliSessionContext::task_ctx(&self.planner)
    }

    fn session_state(&self) -> SessionState {
        CliSessionContext::session_state(&self.planner)
    }

    fn register_object_store(
        &self,
        url: &url::Url,
        object_store: Arc<dyn ObjectStore>,
    ) -> Option<Arc<dyn ObjectStore + 'static>> {
        CliSessionContext::register_object_store(&self.planner, url, object_store)
    }

    fn register_table_options_extension_from_scheme(&self, scheme: &str) {
        CliSessionContext::register_table_options_extension_from_scheme(&self.planner, scheme)
    }

    async fn execute_logical_plan(&self, plan: LogicalPlan) -> Result<DataFrame> {
        self.session.lock().await.execute_cli_plan(plan).await
    }
}

async fn exec_from_validating_repl(
    context: &VectorCliContext,
    print_options: &mut PrintOptions,
) -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut sql = String::new();

    loop {
        print!("{}", if sql.is_empty() { "> " } else { "... " });
        io::stdout().flush().map_err(DataFusionError::IoError)?;

        let mut line = String::new();
        if input
            .read_line(&mut line)
            .map_err(DataFusionError::IoError)?
            == 0
        {
            println!("\\q");
            return Ok(());
        }

        let trimmed = line.trim();
        if sql.is_empty() && trimmed.starts_with('\\') {
            let command = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            match command[1..].parse::<Command>() {
                Ok(Command::Quit) => return Ok(()),
                Ok(Command::Include(Some(filename))) => match fs::read_to_string(&filename) {
                    Ok(sql) => {
                        if let Err(error) =
                            context.execute_sql_continuing(&sql, print_options).await
                        {
                            eprintln!("{error}");
                        }
                    }
                    Err(error) => eprintln!("Error opening {filename:?}: {error}"),
                },
                Ok(Command::OutputFormat(Some(subcommand))) => {
                    match subcommand.parse::<OutputFormat>() {
                        Ok(command) => {
                            if let Err(error) = command.execute(print_options).await {
                                eprintln!("{error}");
                            }
                        }
                        Err(_) => {
                            eprintln!("'{trimmed}' is not a valid command; use '\\?' for help");
                        }
                    }
                }
                Ok(Command::OutputFormat(None)) => {
                    println!("Output format is {:?}.", print_options.format);
                }
                Ok(command) => {
                    if let Err(error) = command.execute(context, print_options).await {
                        eprintln!("{error}");
                    }
                }
                Err(_) => eprintln!("'{trimmed}' is not a valid command; use '\\?' for help"),
            }
            continue;
        }

        sql.push_str(&line);
        if trimmed.ends_with(';')
            && let Err(error) = context
                .execute_sql(std::mem::take(&mut sql), print_options)
                .await
        {
            eprintln!("{error}");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let session = VectorSqlSession::new(
        Metric::Cosine,
        IndexConfig::IvfFlat(IvfFlatConfig {
            partitions: 2,
            probes: 2,
            iterations: 8,
            seed: 7,
        }),
    );
    let context = VectorCliContext::new(session);
    let interactive = io::stdin().is_terminal();
    let mut print_options = PrintOptions {
        format: PrintFormat::Automatic,
        quiet: false,
        maxrows: MaxRows::Unlimited,
        color: io::stdout().is_terminal(),
        instrumented_registry: Arc::new(InstrumentedObjectStoreRegistry::new()),
    };

    println!("DataFusion CLI v{DATAFUSION_CLI_VERSION}");
    if interactive {
        exec_from_validating_repl(&context, &mut print_options).await
    } else {
        let mut sql = String::new();
        io::stdin()
            .read_to_string(&mut sql)
            .map_err(DataFusionError::IoError)?;
        context.execute_sql(sql, &print_options).await
    }
}
