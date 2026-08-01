use crate::graph::{prune_neighbors, search_layer};
use crate::{Dataset, Metric, Neighbor, Result, VectorError, VectorIndex};

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
    pub fn try_new(dataset: Dataset, metric: Metric, config: NswConfig) -> Result<Self> {
        dataset.validate_for_metric(metric)?;
        validate_config(config)?;

        let mut adjacency = Vec::<Vec<usize>>::with_capacity(dataset.len());
        let mut entry_point = 0;
        for row in 0..dataset.len() {
            adjacency.push(Vec::new());
            if row == 0 {
                continue;
            }

            let candidates = search_layer(
                &dataset,
                metric,
                dataset.vector(row),
                &adjacency,
                &[entry_point],
                config.ef_construction.max(config.max_connections),
                row,
            );
            let selected = candidates
                .into_iter()
                .take(config.max_connections)
                .map(|neighbor| neighbor.row)
                .collect::<Vec<_>>();
            adjacency[row].extend(&selected);
            for &neighbor in &selected {
                adjacency[neighbor].push(row);
            }
            let mut affected = selected;
            affected.push(row);
            affected.sort_unstable();
            affected.dedup();
            for owner in affected {
                let previous = adjacency[owner].clone();
                prune_neighbors(
                    &dataset,
                    metric,
                    owner,
                    &mut adjacency[owner],
                    config.max_connections,
                );
                for removed in previous {
                    if !adjacency[owner].contains(&removed) {
                        adjacency[removed].retain(|neighbor| *neighbor != owner);
                    }
                }
            }
            entry_point = row;
        }

        Ok(Self {
            dataset,
            metric,
            config,
            adjacency,
            entry_point,
        })
    }

    pub fn adjacency(&self) -> &[Vec<usize>] {
        &self.adjacency
    }

    pub fn search_with_ef(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
    ) -> Result<Vec<Neighbor>> {
        self.dataset.validate_query(query, self.metric)?;
        if ef_search == 0 {
            return Err(VectorError::InvalidConfig(
                "NSW ef_search must be greater than zero",
            ));
        }
        let mut result = search_layer(
            &self.dataset,
            self.metric,
            query,
            &self.adjacency,
            &[self.entry_point],
            ef_search.max(k),
            self.dataset.len(),
        );
        result.truncate(k.min(result.len()));
        Ok(result)
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

fn validate_config(config: NswConfig) -> Result<()> {
    if config.max_connections == 0 {
        return Err(VectorError::InvalidConfig(
            "NSW max_connections must be greater than zero",
        ));
    }
    if config.ef_construction < config.max_connections {
        return Err(VectorError::InvalidConfig(
            "NSW ef_construction must be at least max_connections",
        ));
    }
    if config.ef_search == 0 {
        return Err(VectorError::InvalidConfig(
            "NSW ef_search must be greater than zero",
        ));
    }
    Ok(())
}
