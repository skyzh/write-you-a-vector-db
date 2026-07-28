use std::time::{Duration, Instant};

use vector_core::{Dataset, FlatIndex, Metric, VectorIndex, recall_at_k};

const ROWS: usize = 2_000;
const DIMENSIONS: usize = 16;
const QUERIES: usize = 100;
const K: usize = 10;

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

    let exact = FlatIndex::try_new(dataset.clone(), Metric::Cosine)?;
    let ground_truth = queries
        .iter()
        .map(|query| exact.search(query, K))
        .collect::<vector_core::Result<Vec<_>>>()?;

    let started = Instant::now();
    let baseline = FlatIndex::try_new(dataset, Metric::Cosine)?;
    let build_time = started.elapsed();
    report(
        "flat_baseline",
        &baseline,
        build_time,
        &queries,
        &ground_truth,
    )?;
    Ok(())
}

fn report(
    name: &str,
    index: &dyn VectorIndex,
    build_time: Duration,
    queries: &[Vec<f32>],
    ground_truth: &[Vec<vector_core::Neighbor>],
) -> vector_core::Result<()> {
    let mut latencies = Vec::with_capacity(queries.len());
    let mut recall = 0.0;
    for (query, expected) in queries.iter().zip(ground_truth) {
        let started = Instant::now();
        let actual = index.search(query, K)?;
        latencies.push(started.elapsed());
        recall += recall_at_k(expected, &actual, K);
    }
    latencies.sort_unstable();
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    println!(
        "{name}: rows={ROWS}, dimensions={DIMENSIONS}, queries={QUERIES}, k={K}, \
         build_ms={:.2}, recall={:.3}, p50_us={:.1}, p99_us={:.1}",
        build_time.as_secs_f64() * 1_000.0,
        recall / queries.len() as f64,
        p50.as_secs_f64() * 1_000_000.0,
        p99.as_secs_f64() * 1_000_000.0,
    );
    Ok(())
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
