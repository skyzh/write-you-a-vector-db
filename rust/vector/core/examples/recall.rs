use std::time::{Duration, Instant};

use vector_core::{
    Dataset, FlatIndex, HnswConfig, HnswIndex, IvfFlatConfig, IvfFlatIndex, Metric, NswConfig,
    NswIndex, VectorIndex, recall_at_k,
};

const ROWS: usize = 2_000;
const DIMENSIONS: usize = 16;
const QUERIES: usize = 100;
const K: usize = 10;

struct Measurement {
    recall: f64,
    p50: Duration,
    p99: Duration,
}

fn main() -> vector_core::Result<()> {
    let dataset = Dataset::try_new(
        (0..ROWS)
            .map(|row| {
                (0..DIMENSIONS)
                    .map(|dimension| sample(row as u64, dimension as u64))
                    .collect()
            })
            .collect(),
    )?;
    let queries = (0..QUERIES)
        .map(|query| {
            (0..DIMENSIONS)
                .map(|dimension| sample((query * 17 + 3) as u64, dimension as u64) + 0.001)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let started = Instant::now();
    let exact = FlatIndex::try_new(dataset.clone(), Metric::Cosine)?;
    let exact_build = started.elapsed();
    let ground_truth = queries
        .iter()
        .map(|query| exact.search(query, K))
        .collect::<vector_core::Result<Vec<_>>>()?;
    report("flat", &exact, exact_build, &queries, &ground_truth)?;

    let started = Instant::now();
    let ivf = IvfFlatIndex::try_new(
        dataset.clone(),
        Metric::Cosine,
        IvfFlatConfig {
            partitions: 32,
            probes: 6,
            iterations: 12,
            seed: 7,
        },
    )?;
    let ivf_build = started.elapsed();
    report("ivf_flat", &ivf, ivf_build, &queries, &ground_truth)?;

    let started = Instant::now();
    let nsw = NswIndex::try_new(
        dataset.clone(),
        Metric::Cosine,
        NswConfig {
            max_connections: 12,
            ef_construction: 64,
            ef_search: 40,
        },
    )?;
    let nsw_build = started.elapsed();
    report("nsw", &nsw, nsw_build, &queries, &ground_truth)?;

    let started = Instant::now();
    let hnsw = HnswIndex::try_new(
        dataset,
        Metric::Cosine,
        HnswConfig {
            max_connections: 12,
            ef_construction: 64,
            ef_search: 40,
            max_level: 12,
            seed: 7,
        },
    )?;
    let hnsw_build = started.elapsed();
    report("hnsw", &hnsw, hnsw_build, &queries, &ground_truth)?;
    Ok(())
}

fn report(
    name: &str,
    index: &dyn VectorIndex,
    build_time: Duration,
    queries: &[Vec<f32>],
    ground_truth: &[Vec<vector_core::Neighbor>],
) -> vector_core::Result<()> {
    warm_up(index, queries)?;

    let measurement = measure(index, queries, ground_truth)?;
    println!(
        "{name}: rows={ROWS}, dimensions={DIMENSIONS}, queries={QUERIES}, k={K}, \
         build_ms={:.2}, recall={:.3}, p50_us={:.1}, p99_us={:.1}",
        build_time.as_secs_f64() * 1_000.0,
        measurement.recall,
        measurement.p50.as_secs_f64() * 1_000_000.0,
        measurement.p99.as_secs_f64() * 1_000_000.0,
    );
    Ok(())
}

fn measure(
    index: &dyn VectorIndex,
    queries: &[Vec<f32>],
    ground_truth: &[Vec<vector_core::Neighbor>],
) -> vector_core::Result<Measurement> {
    let mut latencies = Vec::with_capacity(queries.len());
    let mut recall = 0.0;
    for (query, expected) in queries.iter().zip(ground_truth) {
        let started = Instant::now();
        let actual = std::hint::black_box(index.search(query, K)?);
        latencies.push(started.elapsed());
        recall += recall_at_k(expected, &actual, K);
    }
    latencies.sort_unstable();
    let p50 = percentile(&latencies, 50);
    let p99 = percentile(&latencies, 99);
    Ok(Measurement {
        recall: recall / queries.len() as f64,
        p50,
        p99,
    })
}

fn warm_up(index: &dyn VectorIndex, queries: &[Vec<f32>]) -> vector_core::Result<()> {
    for query in queries {
        std::hint::black_box(index.search(query, K)?);
    }
    Ok(())
}

fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    assert!(!sorted.is_empty());
    assert!(percent <= 100);
    let rank = (percent * sorted.len()).div_ceil(100).saturating_sub(1);
    sorted[rank.min(sorted.len() - 1)]
}

fn sample(row: u64, dimension: u64) -> f32 {
    let mut value = row
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(dimension.wrapping_mul(0x85eb_ca6b));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    let unit = (value & 0xffff) as f32 / 65_535.0;
    unit * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentile_selects_expected_samples() {
        let samples = (1..=100).map(Duration::from_micros).collect::<Vec<_>>();

        assert_eq!(percentile(&samples, 0), Duration::from_micros(1));
        assert_eq!(percentile(&samples, 50), Duration::from_micros(50));
        assert_eq!(percentile(&samples, 99), Duration::from_micros(99));
        assert_eq!(percentile(&samples, 100), Duration::from_micros(100));
    }

    #[test]
    fn flat_measurement_satisfies_report_invariants() {
        let dataset = Dataset::try_new(
            (0..16)
                .map(|row| vec![row as f32, (row % 3) as f32])
                .collect(),
        )
        .unwrap();
        let index = FlatIndex::try_new(dataset, Metric::Euclidean).unwrap();
        let queries = vec![vec![0.25, 1.0], vec![8.5, 2.0], vec![14.2, 0.0]];
        let ground_truth = queries
            .iter()
            .map(|query| index.search(query, K).unwrap())
            .collect::<Vec<_>>();

        let measurement = measure(&index, &queries, &ground_truth).unwrap();

        assert_eq!(measurement.recall, 1.0);
        assert!((0.0..=1.0).contains(&measurement.recall));
        assert!(measurement.p99 >= measurement.p50);
    }
}
