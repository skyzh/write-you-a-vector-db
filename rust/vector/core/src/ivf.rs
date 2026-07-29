use crate::search::{DeterministicRng, TopK};
use crate::{Dataset, Metric, Neighbor, Result, VectorError, VectorIndex};

#[derive(Debug, Clone, Copy)]
pub struct IvfFlatConfig {
    pub partitions: usize,
    pub probes: usize,
    pub iterations: usize,
    pub seed: u64,
}

impl Default for IvfFlatConfig {
    fn default() -> Self {
        Self {
            partitions: 16,
            probes: 4,
            iterations: 12,
            seed: 0x5eed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IvfFlatIndex {
    dataset: Dataset,
    metric: Metric,
    config: IvfFlatConfig,
    centroids: Vec<Vec<f32>>,
    lists: Vec<Vec<usize>>,
}

impl IvfFlatIndex {
    pub fn try_new(dataset: Dataset, metric: Metric, config: IvfFlatConfig) -> Result<Self> {
        dataset.validate_for_metric(metric)?;
        if config.partitions == 0 || config.partitions > dataset.len() {
            return Err(VectorError::InvalidConfig(
                "IVFFlat partitions must be in 1..=dataset length",
            ));
        }
        if config.probes == 0 || config.probes > config.partitions {
            return Err(VectorError::InvalidConfig(
                "IVFFlat probes must be in 1..=partitions",
            ));
        }
        if config.iterations == 0 {
            return Err(VectorError::InvalidConfig(
                "IVFFlat iterations must be greater than zero",
            ));
        }

        let mut rng = DeterministicRng::new(config.seed);
        let mut rows = (0..dataset.len()).collect::<Vec<_>>();
        for end in (1..rows.len()).rev() {
            let selected = rng.index(end + 1);
            rows.swap(end, selected);
        }
        let mut centroids = rows
            .into_iter()
            .take(config.partitions)
            .map(|row| dataset.vector(row).to_vec())
            .collect::<Vec<_>>();

        let mut assignments = vec![usize::MAX; dataset.len()];
        for _ in 0..config.iterations {
            let next_assignments = dataset
                .vectors()
                .iter()
                .map(|vector| nearest_centroid(metric, vector, &centroids))
                .collect::<Vec<_>>();
            if next_assignments == assignments {
                break;
            }
            assignments = next_assignments;

            let mut sums = vec![vec![0.0_f64; dataset.dimension()]; config.partitions];
            let mut counts = vec![0_usize; config.partitions];
            for (row, &partition) in assignments.iter().enumerate() {
                counts[partition] += 1;
                for (sum, value) in sums[partition].iter_mut().zip(dataset.vector(row)) {
                    *sum += f64::from(*value);
                }
            }

            for partition in 0..config.partitions {
                if counts[partition] == 0 {
                    let row = farthest_row(&dataset, metric, &centroids);
                    centroids[partition].clone_from_slice(dataset.vector(row));
                    continue;
                }
                for (centroid, sum) in centroids[partition].iter_mut().zip(&sums[partition]) {
                    *centroid = (*sum / counts[partition] as f64) as f32;
                }
                if !normalize_centroid(metric, &mut centroids[partition]) {
                    let row = assignments
                        .iter()
                        .position(|assigned| *assigned == partition)
                        .expect("a non-empty partition has an assigned row");
                    centroids[partition].clone_from_slice(dataset.vector(row));
                }
            }
        }

        let mut lists = vec![Vec::new(); config.partitions];
        for (row, vector) in dataset.vectors().iter().enumerate() {
            lists[nearest_centroid(metric, vector, &centroids)].push(row);
        }

        Ok(Self {
            dataset,
            metric,
            config,
            centroids,
            lists,
        })
    }

    pub fn centroids(&self) -> &[Vec<f32>] {
        &self.centroids
    }

    pub fn list_sizes(&self) -> Vec<usize> {
        self.lists.iter().map(Vec::len).collect()
    }

    pub fn search_with_probes(
        &self,
        query: &[f32],
        k: usize,
        probes: usize,
    ) -> Result<Vec<Neighbor>> {
        self.dataset.validate_query(query, self.metric)?;
        if probes == 0 || probes > self.centroids.len() {
            return Err(VectorError::InvalidConfig(
                "query probes must be in 1..=partitions",
            ));
        }

        let mut partitions = self
            .centroids
            .iter()
            .enumerate()
            .map(|(row, centroid)| Neighbor {
                row,
                distance: self.metric.distance(query, centroid),
            })
            .collect::<Vec<_>>();
        partitions.sort_unstable();

        let mut result = TopK::new(k.min(self.dataset.len()));
        for partition in partitions.iter().take(probes) {
            for &row in &self.lists[partition.row] {
                result.push(Neighbor {
                    row,
                    distance: self.metric.distance(query, self.dataset.vector(row)),
                });
            }
        }
        Ok(result.into_sorted())
    }
}

impl VectorIndex for IvfFlatIndex {
    fn kind(&self) -> &'static str {
        "ivf_flat"
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn metric(&self) -> Metric {
        self.metric
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<Neighbor>> {
        self.search_with_probes(query, k, self.config.probes)
    }
}

fn nearest_centroid(metric: Metric, vector: &[f32], centroids: &[Vec<f32>]) -> usize {
    centroids
        .iter()
        .enumerate()
        .map(|(row, centroid)| Neighbor {
            row,
            distance: metric.distance(vector, centroid),
        })
        .min()
        .expect("IVFFlat always has at least one centroid")
        .row
}

fn farthest_row(dataset: &Dataset, metric: Metric, centroids: &[Vec<f32>]) -> usize {
    dataset
        .vectors()
        .iter()
        .enumerate()
        .map(|(row, vector)| {
            let distance = centroids
                .iter()
                .map(|centroid| metric.distance(vector, centroid))
                .min_by(f32::total_cmp)
                .unwrap_or(f32::INFINITY);
            Neighbor { row, distance }
        })
        .max()
        .expect("dataset is non-empty")
        .row
}

fn normalize_centroid(metric: Metric, centroid: &mut [f32]) -> bool {
    if metric != Metric::Cosine {
        return true;
    }
    let norm = Metric::squared_norm(centroid).sqrt();
    if norm == 0.0 {
        return false;
    }
    for value in centroid {
        *value = (f64::from(*value) / norm) as f32;
    }
    true
}
