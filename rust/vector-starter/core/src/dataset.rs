use std::sync::Arc;

use crate::{Metric, Result};

#[derive(Debug, Clone)]
pub struct Dataset {
    vectors: Arc<[Vec<f32>]>,
    dimension: usize,
}

impl Dataset {
    pub fn try_new(_vectors: Vec<Vec<f32>>) -> Result<Self> {
        todo!("Chapter 1: validate a non-empty, rectangular, finite dataset")
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn vector(&self, row: usize) -> &[f32] {
        &self.vectors[row]
    }

    pub fn vectors(&self) -> &[Vec<f32>] {
        &self.vectors
    }

    pub(crate) fn validate_for_metric(&self, _metric: Metric) -> Result<()> {
        todo!("Chapter 1: reject zero-norm dataset rows for cosine distance")
    }

    pub(crate) fn validate_query(&self, _query: &[f32], _metric: Metric) -> Result<()> {
        todo!("Chapter 1: validate query dimension, finiteness, and cosine norm")
    }
}
