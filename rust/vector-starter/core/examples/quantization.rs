use std::time::{Duration, Instant};

use vector_core_starter::{
    Dataset, FlatIndex, IvfFlatConfig, IvfFlatIndex, IvfPqConfig, IvfPqIndex, Metric, VectorIndex,
    recall_at_k,
};

const ROWS: usize = 5_000;
const DIMENSIONS: usize = 128;
const QUERIES: usize = 100;
const K: usize = 10;

fn main() -> vector_core_starter::Result<()> {
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

    let exact = FlatIndex::try_new(dataset.clone(), Metric::Euclidean)?;
    let ground_truth = queries
        .iter()
        .map(|query| exact.search(query, K))
        .collect::<vector_core_starter::Result<Vec<_>>>()?;

    let started = Instant::now();
    let ivf = IvfFlatIndex::try_new(
        dataset.clone(),
        Metric::Euclidean,
        IvfFlatConfig {
            partitions: 64,
            probes: 8,
            iterations: 12,
            seed: 7,
        },
    )?;
    let ivf_build = started.elapsed();
    report("ivf_flat", &ivf, ivf_build, &queries, &ground_truth)?;

    let started = Instant::now();
    let ivf_pq = IvfPqIndex::try_new(
        dataset,
        Metric::Euclidean,
        IvfPqConfig {
            partitions: 64,
            probes: 8,
            iterations: 12,
            subquantizers: 16,
            codebook_size: 16,
            rerank: 100,
            seed: 7,
        },
    )?;
    let ivf_pq_build = started.elapsed();
    report("ivf_pq", &ivf_pq, ivf_pq_build, &queries, &ground_truth)?;
    let compressed = ivf_pq.encoded_bytes() + ivf_pq.codebook_bytes();
    println!(
        "ivf_pq search representation: codes_bytes={}, codebooks_bytes={}, full_vectors_bytes={}, compression={:.1}x",
        ivf_pq.encoded_bytes(),
        ivf_pq.codebook_bytes(),
        ivf_pq.full_precision_bytes(),
        ivf_pq.full_precision_bytes() as f64 / compressed as f64,
    );
    Ok(())
}

fn report(
    name: &str,
    index: &dyn VectorIndex,
    build_time: Duration,
    queries: &[Vec<f32>],
    ground_truth: &[Vec<vector_core_starter::Neighbor>],
) -> vector_core_starter::Result<()> {
    for query in queries {
        std::hint::black_box(index.search(query, K)?);
    }

    let mut latencies = Vec::with_capacity(queries.len());
    let mut recall = 0.0;
    for (query, expected) in queries.iter().zip(ground_truth) {
        let started = Instant::now();
        let actual = std::hint::black_box(index.search(query, K)?);
        latencies.push(started.elapsed());
        recall += recall_at_k(expected, &actual, K);
    }
    latencies.sort_unstable();
    println!(
        "{name}: build_ms={:.2}, recall={:.3}, p50_us={:.1}, p99_us={:.1}",
        build_time.as_secs_f64() * 1_000.0,
        recall / queries.len() as f64,
        percentile(&latencies, 50).as_secs_f64() * 1_000_000.0,
        percentile(&latencies, 99).as_secs_f64() * 1_000_000.0,
    );
    Ok(())
}

fn percentile(sorted: &[Duration], percent: usize) -> Duration {
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
