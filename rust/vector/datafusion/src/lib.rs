mod attachment;
mod optimizer;
mod sql;
mod table;

pub use attachment::VectorIndexAttachment;
pub use optimizer::{VectorSearchOptions, with_vector_indexes, with_vector_search_options};
pub use sql::{VectorSqlOutput, VectorSqlSession};
pub use table::{VectorRow, vector_mem_table};

use datafusion::common::DataFusionError;

pub(crate) fn core_error(error: vector_core::VectorError) -> DataFusionError {
    DataFusionError::Plan(error.to_string())
}
