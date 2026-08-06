use std::mem::size_of;

use crate::search::{DeterministicRng, TopK};
use crate::{
    Dataset, IvfFlatConfig, IvfFlatIndex, Metric, Neighbor, Result, VectorError, VectorIndex,
};

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
    pub fn try_new(dataset: Dataset, metric: Metric, config: IvfPqConfig) -> Result<Self> {
        dataset.validate_for_metric(metric)?;
        if metric != Metric::Euclidean {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ currently supports Euclidean distance only",
            ));
        }
        if config.partitions == 0 || config.partitions > dataset.len() {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ partitions must be in 1..=dataset length",
            ));
        }
        if config.probes == 0 || config.probes > config.partitions {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ probes must be in 1..=partitions",
            ));
        }
        if config.iterations == 0 {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ iterations must be greater than zero",
            ));
        }
        if config.subquantizers == 0 || !dataset.dimension().is_multiple_of(config.subquantizers) {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ subquantizers must divide the vector dimension",
            ));
        }
        if config.codebook_size < 2
            || config.codebook_size > 256
            || config.codebook_size > dataset.len()
        {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ codebook size must be in 2..=min(256, dataset length)",
            ));
        }
        if config.rerank == 0 {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ rerank must be greater than zero",
            ));
        }

        let ivf = IvfFlatIndex::try_new(
            dataset.clone(),
            metric,
            IvfFlatConfig {
                partitions: config.partitions,
                probes: config.probes,
                iterations: config.iterations,
                seed: config.seed,
            },
        )?;
        let centroids = ivf.centroids().to_vec();
        let assignments = dataset
            .vectors()
            .iter()
            .map(|vector| nearest_vector(vector, &centroids))
            .collect::<Vec<_>>();
        let residuals = dataset
            .vectors()
            .iter()
            .zip(&assignments)
            .map(|(vector, &partition)| {
                residual(vector, &centroids[partition]).ok_or(VectorError::InvalidConfig(
                    "IVF-PQ training residuals must remain finite",
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let subvector_dimension = dataset.dimension() / config.subquantizers;
        let codebooks = (0..config.subquantizers)
            .map(|subquantizer| {
                let start = subquantizer * subvector_dimension;
                let end = start + subvector_dimension;
                train_codebook(
                    &residuals,
                    start,
                    end,
                    config.codebook_size,
                    config.iterations,
                    config.seed.wrapping_add(
                        (subquantizer as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15),
                    ),
                )
            })
            .collect::<Vec<_>>();
        if codebooks
            .iter()
            .flatten()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(VectorError::InvalidConfig(
                "IVF-PQ training codebooks must remain finite",
            ));
        }

        let mut lists = vec![Vec::new(); config.partitions];
        for (row, (&partition, residual)) in assignments.iter().zip(&residuals).enumerate() {
            lists[partition].push(QuantizedRow {
                row,
                codes: encode(residual, &codebooks, subvector_dimension),
            });
        }

        Ok(Self {
            dataset,
            config,
            centroids,
            codebooks,
            lists,
        })
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
        query: &[f32],
        k: usize,
        probes: usize,
        rerank: usize,
    ) -> Result<Vec<Neighbor>> {
        self.dataset.validate_query(query, Metric::Euclidean)?;
        if probes == 0 || probes > self.centroids.len() {
            return Err(VectorError::InvalidConfig(
                "query probes must be in 1..=partitions",
            ));
        }
        if rerank == 0 {
            return Err(VectorError::InvalidConfig(
                "query rerank must be greater than zero",
            ));
        }
        if k == 0 {
            return Ok(Vec::new());
        }

        let mut partitions = self
            .centroids
            .iter()
            .enumerate()
            .map(|(row, centroid)| Neighbor {
                row,
                distance: Metric::Euclidean.distance(query, centroid),
            })
            .collect::<Vec<_>>();
        partitions.sort_unstable();

        let shortlist_size = rerank.max(k).min(self.dataset.len());
        let mut shortlist = TopK::new(shortlist_size);
        let subvector_dimension = self.dataset.dimension() / self.config.subquantizers;
        let mut tables = vec![0.0; self.config.subquantizers * self.config.codebook_size];
        for partition in partitions.iter().take(probes) {
            fill_lookup_tables(
                &mut tables,
                query,
                &self.centroids[partition.row],
                &self.codebooks,
                subvector_dimension,
                self.config.codebook_size,
            );
            for encoded in &self.lists[partition.row] {
                let distance = encoded
                    .codes
                    .iter()
                    .enumerate()
                    .map(|(subquantizer, code)| {
                        tables[subquantizer * self.config.codebook_size + usize::from(*code)]
                    })
                    .sum::<f32>();
                shortlist.push(Neighbor {
                    row: encoded.row,
                    distance,
                });
            }
        }

        let mut result = TopK::new(k.min(self.dataset.len()));
        for candidate in shortlist.into_sorted() {
            result.push(Neighbor {
                row: candidate.row,
                distance: Metric::Euclidean.distance(query, self.dataset.vector(candidate.row)),
            });
        }
        Ok(result.into_sorted())
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

fn train_codebook(
    residuals: &[Vec<f32>],
    start: usize,
    end: usize,
    codebook_size: usize,
    iterations: usize,
    seed: u64,
) -> Vec<Vec<f32>> {
    let mut rng = DeterministicRng::new(seed);
    let mut rows = (0..residuals.len()).collect::<Vec<_>>();
    for end in (1..rows.len()).rev() {
        let selected = rng.index(end + 1);
        rows.swap(end, selected);
    }
    let mut centroids = rows
        .into_iter()
        .take(codebook_size)
        .map(|row| residuals[row][start..end].to_vec())
        .collect::<Vec<_>>();

    let mut assignments = vec![usize::MAX; residuals.len()];
    for _ in 0..iterations {
        let next_assignments = residuals
            .iter()
            .map(|vector| nearest_vector(&vector[start..end], &centroids))
            .collect::<Vec<_>>();
        if next_assignments == assignments {
            break;
        }
        assignments = next_assignments;

        let mut sums = vec![vec![0.0_f64; end - start]; codebook_size];
        let mut counts = vec![0_usize; codebook_size];
        for (vector, &code) in residuals.iter().zip(&assignments) {
            counts[code] += 1;
            for (sum, value) in sums[code].iter_mut().zip(&vector[start..end]) {
                *sum += f64::from(*value);
            }
        }
        for code in 0..codebook_size {
            if counts[code] == 0 {
                continue;
            }
            for (centroid, sum) in centroids[code].iter_mut().zip(&sums[code]) {
                *centroid = (*sum / counts[code] as f64) as f32;
            }
        }
    }
    centroids
}

fn encode(residual: &[f32], codebooks: &[Vec<Vec<f32>>], subvector_dimension: usize) -> Vec<u8> {
    codebooks
        .iter()
        .enumerate()
        .map(|(subquantizer, codebook)| {
            let start = subquantizer * subvector_dimension;
            let end = start + subvector_dimension;
            nearest_vector(&residual[start..end], codebook) as u8
        })
        .collect()
}

fn fill_lookup_tables(
    tables: &mut [f32],
    query: &[f32],
    coarse_centroid: &[f32],
    codebooks: &[Vec<Vec<f32>>],
    subvector_dimension: usize,
    codebook_size: usize,
) {
    for (subquantizer, codebook) in codebooks.iter().enumerate() {
        let start = subquantizer * subvector_dimension;
        for (code, centroid) in codebook.iter().enumerate() {
            let distance = query[start..start + subvector_dimension]
                .iter()
                .zip(&coarse_centroid[start..start + subvector_dimension])
                .zip(centroid)
                .map(|((query, coarse), codeword)| {
                    let delta = f64::from(*query) - f64::from(*coarse) - f64::from(*codeword);
                    delta * delta
                })
                .sum::<f64>() as f32;
            tables[subquantizer * codebook_size + code] = distance;
        }
    }
}

fn residual(vector: &[f32], centroid: &[f32]) -> Option<Vec<f32>> {
    vector
        .iter()
        .zip(centroid)
        .map(|(value, centroid)| {
            let delta = f64::from(*value) - f64::from(*centroid);
            if delta < f64::from(f32::MIN) || delta > f64::from(f32::MAX) {
                None
            } else {
                Some(delta as f32)
            }
        })
        .collect()
}

fn nearest_vector(vector: &[f32], centroids: &[Vec<f32>]) -> usize {
    centroids
        .iter()
        .enumerate()
        .map(|(index, centroid)| (index, squared_l2(vector, centroid)))
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })
        .expect("a trained quantizer has at least one centroid")
        .0
}

fn squared_l2(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = f64::from(*left) - f64::from(*right);
            delta * delta
        })
        .sum::<f64>() as f32
}
