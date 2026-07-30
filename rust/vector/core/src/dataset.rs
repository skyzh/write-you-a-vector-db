use std::sync::Arc;

use crate::{Metric, Result, VectorError};

#[derive(Debug, Clone)]
pub struct Dataset {
    vectors: Arc<[Vec<f32>]>,
    dimension: usize,
}

impl Dataset {
    pub fn try_new(vectors: Vec<Vec<f32>>) -> Result<Self> {
        let Some(first) = vectors.first() else {
            return Err(VectorError::EmptyDataset);
        };
        let dimension = first.len();
        if dimension == 0 {
            return Err(VectorError::EmptyVector);
        }

        for (vector_idx, vector) in vectors.iter().enumerate() {
            if vector.len() != dimension {
                return Err(VectorError::DimensionMismatch {
                    expected: dimension,
                    actual: vector.len(),
                });
            }
            if let Some(dimension) = vector.iter().position(|value| !value.is_finite()) {
                return Err(VectorError::NonFiniteValue {
                    vector: vector_idx,
                    dimension,
                });
            }
        }

        Ok(Self {
            vectors: vectors.into(),
            dimension,
        })
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

    pub(crate) fn validate_for_metric(&self, metric: Metric) -> Result<()> {
        if metric != Metric::Cosine {
            return Ok(());
        }
        for (row, vector) in self.vectors.iter().enumerate() {
            if Metric::squared_norm(vector) == 0.0 {
                return Err(VectorError::ZeroNorm { vector: row });
            }
        }
        Ok(())
    }

    pub(crate) fn validate_query(&self, query: &[f32], metric: Metric) -> Result<()> {
        if query.len() != self.dimension {
            return Err(VectorError::DimensionMismatch {
                expected: self.dimension,
                actual: query.len(),
            });
        }
        if let Some(dimension) = query.iter().position(|value| !value.is_finite()) {
            return Err(VectorError::NonFiniteValue {
                vector: self.len(),
                dimension,
            });
        }
        if metric == Metric::Cosine && Metric::squared_norm(query) == 0.0 {
            return Err(VectorError::ZeroNorm { vector: self.len() });
        }
        Ok(())
    }
}
