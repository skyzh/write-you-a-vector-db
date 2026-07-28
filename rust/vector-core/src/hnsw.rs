use crate::graph::{greedy_search, prune_neighbors, search_layer};
use crate::search::DeterministicRng;
use crate::{Dataset, Metric, Neighbor, Result, VectorError, VectorIndex};

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
    pub fn try_new(dataset: Dataset, metric: Metric, config: HnswConfig) -> Result<Self> {
        dataset.validate_for_metric(metric)?;
        validate_config(config)?;

        let mut rng = DeterministicRng::new(config.seed);
        let mut levels = Vec::with_capacity(dataset.len());
        let mut layers = Vec::<Vec<Vec<usize>>>::new();
        let mut entry_point = 0;
        let mut top_level = 0;

        for row in 0..dataset.len() {
            let level = random_level(&mut rng, config.max_level);
            levels.push(level);
            for layer in &mut layers {
                layer.push(Vec::new());
            }
            while layers.len() <= level {
                layers.push(vec![Vec::new(); row + 1]);
            }

            if row == 0 {
                entry_point = row;
                top_level = level;
                continue;
            }

            let query = dataset.vector(row);
            let mut nearest_entry = entry_point;
            if top_level > level {
                for current_level in ((level + 1)..=top_level).rev() {
                    nearest_entry = greedy_search(
                        &dataset,
                        metric,
                        query,
                        &layers[current_level],
                        nearest_entry,
                        row,
                    );
                }
            }

            for current_level in (0..=level.min(top_level)).rev() {
                let candidates = search_layer(
                    &dataset,
                    metric,
                    query,
                    &layers[current_level],
                    &[nearest_entry],
                    config.ef_construction.max(config.max_connections),
                    row,
                );
                let selected = candidates
                    .iter()
                    .take(config.max_connections)
                    .map(|neighbor| neighbor.row)
                    .collect::<Vec<_>>();
                layers[current_level][row].extend(&selected);
                for &neighbor in &selected {
                    layers[current_level][neighbor].push(row);
                    prune_neighbors(
                        &dataset,
                        metric,
                        neighbor,
                        &mut layers[current_level][neighbor],
                        config.max_connections,
                    );
                }
                prune_neighbors(
                    &dataset,
                    metric,
                    row,
                    &mut layers[current_level][row],
                    config.max_connections,
                );
                if let Some(nearest) = candidates.first() {
                    nearest_entry = nearest.row;
                }
            }

            if level > top_level {
                entry_point = row;
                top_level = level;
            }
        }

        Ok(Self {
            dataset,
            metric,
            config,
            levels,
            layers,
            entry_point,
            top_level,
        })
    }

    pub fn levels(&self) -> &[usize] {
        &self.levels
    }

    pub fn top_level(&self) -> usize {
        self.top_level
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
                "HNSW ef_search must be greater than zero",
            ));
        }

        let mut nearest_entry = self.entry_point;
        for level in (1..=self.top_level).rev() {
            nearest_entry = greedy_search(
                &self.dataset,
                self.metric,
                query,
                &self.layers[level],
                nearest_entry,
                self.dataset.len(),
            );
        }
        let mut result = search_layer(
            &self.dataset,
            self.metric,
            query,
            &self.layers[0],
            &[nearest_entry],
            ef_search.max(k),
            self.dataset.len(),
        );
        result.truncate(k.min(result.len()));
        Ok(result)
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

fn validate_config(config: HnswConfig) -> Result<()> {
    if config.max_connections == 0 {
        return Err(VectorError::InvalidConfig(
            "HNSW max_connections must be greater than zero",
        ));
    }
    if config.ef_construction < config.max_connections {
        return Err(VectorError::InvalidConfig(
            "HNSW ef_construction must be at least max_connections",
        ));
    }
    if config.ef_search == 0 {
        return Err(VectorError::InvalidConfig(
            "HNSW ef_search must be greater than zero",
        ));
    }
    if config.max_level == 0 {
        return Err(VectorError::InvalidConfig(
            "HNSW max_level must be greater than zero",
        ));
    }
    Ok(())
}

fn random_level(rng: &mut DeterministicRng, max_level: usize) -> usize {
    let mut level = 0;
    while level < max_level && rng.coin_flip() {
        level += 1;
    }
    level
}
