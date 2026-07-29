#![allow(dead_code, unused_imports)]

mod dataset;
mod flat;
mod ivf;
mod metric;
mod search;

use std::sync::Arc;

pub use dataset::Dataset;
pub use flat::FlatIndex;
pub use ivf::{IvfFlatConfig, IvfFlatIndex};
pub use metric::Metric;
pub use search::{Neighbor, recall_at_k};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VectorError {
    EmptyDataset,
    EmptyVector,
    DimensionMismatch { expected: usize, actual: usize },
    NonFiniteValue { vector: usize, dimension: usize },
    ZeroNorm { vector: usize },
    InvalidConfig(&'static str),
}

impl std::fmt::Display for VectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDataset => write!(f, "a vector dataset must contain at least one row"),
            Self::EmptyVector => write!(f, "vector dimension must be greater than zero"),
            Self::DimensionMismatch { expected, actual } => {
                write!(f, "expected dimension {expected}, got {actual}")
            }
            Self::NonFiniteValue { vector, dimension } => write!(
                f,
                "vector {vector} contains a non-finite value at dimension {dimension}"
            ),
            Self::ZeroNorm { vector } => {
                write!(f, "vector {vector} has zero norm under cosine distance")
            }
            Self::InvalidConfig(message) => write!(f, "invalid index configuration: {message}"),
        }
    }
}

impl std::error::Error for VectorError {}

pub type Result<T> = std::result::Result<T, VectorError>;

pub trait VectorIndex: std::fmt::Debug + Send + Sync {
    fn kind(&self) -> &'static str;
    fn dataset(&self) -> &Dataset;
    fn metric(&self) -> Metric;
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<Neighbor>>;
}

#[derive(Debug, Clone)]
pub enum IndexConfig {
    Flat,
    IvfFlat(IvfFlatConfig),
}

impl IndexConfig {
    pub fn build(self, dataset: Dataset, metric: Metric) -> Result<Arc<dyn VectorIndex>> {
        match self {
            Self::Flat => Ok(Arc::new(FlatIndex::try_new(dataset, metric)?)),
            Self::IvfFlat(config) => Ok(Arc::new(IvfFlatIndex::try_new(dataset, metric, config)?)),
        }
    }
}
