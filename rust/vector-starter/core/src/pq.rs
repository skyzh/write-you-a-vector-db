use std::mem::size_of;

use crate::{Dataset, Metric, Neighbor, Result, VectorIndex};

#[derive(Debug, Clone, Copy)]
pub struct IvfPqConfig {
    pub partitions: usize,
    pub probes: usize,
    pub iterations: usize,
    pub subquantizers: usize,
    pub codebook_size: usize,
    pub rerank: usize,
    pub seed: u64,
}

impl Default for IvfPqConfig {
    fn default() -> Self {
        Self {
            partitions: 16,
            probes: 4,
            iterations: 12,
            subquantizers: 4,
            codebook_size: 16,
            rerank: 32,
            seed: 0x5eed,
        }
    }
}

#[derive(Debug, Clone)]
struct QuantizedRow {
    row: usize,
    codes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct IvfPqIndex {
    dataset: Dataset,
    config: IvfPqConfig,
    centroids: Vec<Vec<f32>>,
    codebooks: Vec<Vec<Vec<f32>>>,
    lists: Vec<Vec<QuantizedRow>>,
}

impl IvfPqIndex {
    pub fn try_new(_dataset: Dataset, _metric: Metric, _config: IvfPqConfig) -> Result<Self> {
        todo!("Chapter 5: train IVF-PQ and encode every residual")
    }

    pub fn centroids(&self) -> &[Vec<f32>] {
        &self.centroids
    }

    pub fn codebooks(&self) -> &[Vec<Vec<f32>>] {
        &self.codebooks
    }

    pub fn list_sizes(&self) -> Vec<usize> {
        self.lists.iter().map(Vec::len).collect()
    }

    pub fn encoded_bytes(&self) -> usize {
        self.lists.iter().flatten().map(|row| row.codes.len()).sum()
    }

    pub fn codebook_bytes(&self) -> usize {
        self.codebooks
            .iter()
            .flatten()
            .map(|centroid| centroid.len() * size_of::<f32>())
            .sum()
    }

    pub fn full_precision_bytes(&self) -> usize {
        self.dataset.len() * self.dataset.dimension() * size_of::<f32>()
    }

    pub fn search_with_probes(
        &self,
        _query: &[f32],
        _k: usize,
        _probes: usize,
        _rerank: usize,
    ) -> Result<Vec<Neighbor>> {
        todo!("Chapter 5: scan PQ codes with lookup tables and rerank candidates")
    }
}

impl VectorIndex for IvfPqIndex {
    fn kind(&self) -> &'static str {
        "ivf_pq"
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn metric(&self) -> Metric {
        Metric::Euclidean
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<Neighbor>> {
        self.search_with_probes(query, k, self.config.probes, self.config.rerank)
    }
}
