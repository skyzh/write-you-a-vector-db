use crate::{Dataset, Metric, Neighbor, Result, VectorIndex};

#[derive(Debug, Clone, Copy)]
pub struct NswConfig {
    pub max_connections: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
}

impl Default for NswConfig {
    fn default() -> Self {
        Self {
            max_connections: 12,
            ef_construction: 48,
            ef_search: 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NswIndex {
    dataset: Dataset,
    metric: Metric,
    config: NswConfig,
    adjacency: Vec<Vec<usize>>,
    entry_point: usize,
}

impl NswIndex {
    pub fn try_new(_dataset: Dataset, _metric: Metric, _config: NswConfig) -> Result<Self> {
        todo!("Chapter 3: insert rows into a bounded reciprocal proximity graph")
    }

    pub fn adjacency(&self) -> &[Vec<usize>] {
        &self.adjacency
    }

    pub fn search_with_ef(
        &self,
        _query: &[f32],
        _k: usize,
        _ef_search: usize,
    ) -> Result<Vec<Neighbor>> {
        todo!("Chapter 3: search the NSW graph with ef_search.max(k)")
    }
}

impl VectorIndex for NswIndex {
    fn kind(&self) -> &'static str {
        "nsw"
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
