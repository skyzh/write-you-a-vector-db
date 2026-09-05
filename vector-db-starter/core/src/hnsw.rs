use crate::{Dataset, Metric, Neighbor, Result, VectorIndex};

#[derive(Debug, Clone, Copy)]
pub struct HnswConfig {
    pub max_connections: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub max_level: usize,
    pub seed: u64,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            max_connections: 12,
            ef_construction: 64,
            ef_search: 40,
            max_level: 16,
            seed: 0x5eed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HnswIndex {
    dataset: Dataset,
    metric: Metric,
    config: HnswConfig,
    levels: Vec<usize>,
    layers: Vec<Vec<Vec<usize>>>,
    entry_point: usize,
    top_level: usize,
}

impl HnswIndex {
    pub fn try_new(_dataset: Dataset, _metric: Metric, _config: HnswConfig) -> Result<Self> {
        todo!("Day 4: assign seeded levels and build every included graph layer")
    }

    pub fn levels(&self) -> &[usize] {
        &self.levels
    }

    pub fn top_level(&self) -> usize {
        self.top_level
    }

    pub fn search_with_ef(
        &self,
        _query: &[f32],
        _k: usize,
        _ef_search: usize,
    ) -> Result<Vec<Neighbor>> {
        todo!("Day 4: descend greedily and search layer zero with ef_search.max(k)")
    }

    pub fn layer(&self, level: usize) -> Option<&[Vec<usize>]> {
        self.layers.get(level).map(Vec::as_slice)
    }
}

impl VectorIndex for HnswIndex {
    fn kind(&self) -> &'static str {
        "hnsw"
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn metric(&self) -> Metric {
        self.metric
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<Neighbor>> {
        self.search_with_ef(query, k, self.config.ef_search)
    }
}
