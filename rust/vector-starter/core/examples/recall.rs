use std::time::{Duration, Instant};

use vector_core_starter::{
    Dataset, FlatIndex, HnswConfig, HnswIndex, IvfFlatConfig, IvfFlatIndex, IvfPqConfig,
    IvfPqIndex, Metric, Neighbor, NswConfig, NswIndex, VectorIndex, recall_at_k,
};

const ROWS: usize = 2_000;
const DIMENSIONS: usize = 16;
const QUERIES: usize = 100;
const K: usize = 10;
const INDEX_COUNT: usize = 5;
const QUERY_STRIDE: usize = 17;
const QUERY_OFFSET: usize = 3;
const INDEX_NAMES: [&str; INDEX_COUNT] = ["flat", "ivf_flat", "nsw", "hnsw", "ivf_pq"];

struct Workload {
    dataset: Dataset,
    queries: Vec<Vec<f32>>,
    metric: Metric,
    k: usize,
}

impl Workload {
    fn fixed() -> vector_core_starter::Result<Self> {
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
                    .map(|dimension| {
                        sample(
                            (ROWS + query * QUERY_STRIDE + QUERY_OFFSET) as u64,
                            dimension as u64,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        Ok(Self {
            dataset,
            queries,
            metric: Metric::Euclidean,
            k: K,
        })
    }
}

struct BuiltIndex {
    name: &'static str,
    index: Box<dyn VectorIndex>,
    build_time: Duration,
}

#[derive(Debug)]
struct TimedRun {
    latencies: Vec<Duration>,
    results: Vec<Vec<Neighbor>>,
}

#[derive(Debug, Clone, Copy)]
struct Measurement {
    recall: f64,
    p50: Duration,
    p99: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PqAccounting {
    codes_bytes: usize,
    codebooks_bytes: usize,
    full_vectors_bytes: usize,
}

impl PqAccounting {
    fn from_index(index: &IvfPqIndex) -> Self {
        Self {
            codes_bytes: index.encoded_bytes(),
            codebooks_bytes: index.codebook_bytes(),
            full_vectors_bytes: index.full_precision_bytes(),
        }
    }

    fn search_bytes(self) -> usize {
        self.codes_bytes + self.codebooks_bytes
    }

    fn compression(self) -> f64 {
        self.full_vectors_bytes as f64 / self.search_bytes() as f64
    }
}

fn main() -> vector_core_starter::Result<()> {
    let workload = Workload::fixed()?;

    let dataset = workload.dataset.clone();
    let metric = workload.metric;
    let started = Instant::now();
    let flat = FlatIndex::try_new(dataset, metric)?;
    let flat_build = started.elapsed();
    let ground_truth = workload
        .queries
        .iter()
        .map(|query| flat.search(query, workload.k))
        .collect::<vector_core_starter::Result<Vec<_>>>()?;

    let dataset = workload.dataset.clone();
    let metric = workload.metric;
    let config = ivf_flat_config();
    let started = Instant::now();
    let ivf_flat = IvfFlatIndex::try_new(dataset, metric, config)?;
    let ivf_flat_build = started.elapsed();

    let dataset = workload.dataset.clone();
    let metric = workload.metric;
    let config = nsw_config();
    let started = Instant::now();
    let nsw = build_nsw(dataset, metric, config)?;
    let nsw_build = started.elapsed();

    let dataset = workload.dataset.clone();
    let metric = workload.metric;
    let config = hnsw_config();
    let started = Instant::now();
    let hnsw = build_hnsw(dataset, metric, config)?;
    let hnsw_build = started.elapsed();

    let dataset = workload.dataset.clone();
    let metric = workload.metric;
    let config = ivf_pq_config();
    let started = Instant::now();
    let ivf_pq = build_ivf_pq(dataset, metric, config)?;
    let ivf_pq_build = started.elapsed();
    let accounting = PqAccounting::from_index(&ivf_pq);

    let indexes = vec![
        BuiltIndex {
            name: "flat",
            index: Box::new(flat),
            build_time: flat_build,
        },
        BuiltIndex {
            name: "ivf_flat",
            index: Box::new(ivf_flat),
            build_time: ivf_flat_build,
        },
        BuiltIndex {
            name: "nsw",
            index: Box::new(nsw),
            build_time: nsw_build,
        },
        BuiltIndex {
            name: "hnsw",
            index: Box::new(hnsw),
            build_time: hnsw_build,
        },
        BuiltIndex {
            name: "ivf_pq",
            index: Box::new(ivf_pq),
            build_time: ivf_pq_build,
        },
    ];

    warm_up(&indexes, &workload)?;
    let timed_runs = measure(&indexes, &workload)?;
    for line in format_report(&workload, &indexes, &timed_runs, &ground_truth, accounting) {
        println!("{line}");
    }
    Ok(())
}

fn ivf_flat_config() -> IvfFlatConfig {
    IvfFlatConfig {
        partitions: 32,
        probes: 6,
        iterations: 12,
        seed: 7,
    }
}

fn nsw_config() -> NswConfig {
    NswConfig {
        max_connections: 12,
        ef_construction: 64,
        ef_search: 40,
    }
}

fn hnsw_config() -> HnswConfig {
    HnswConfig {
        max_connections: 12,
        ef_construction: 64,
        ef_search: 40,
        max_level: 12,
        seed: 7,
    }
}

fn ivf_pq_config() -> IvfPqConfig {
    IvfPqConfig {
        partitions: 32,
        probes: 6,
        iterations: 12,
        subquantizers: 4,
        codebook_size: 16,
        rerank: 100,
        seed: 7,
    }
}

fn build_nsw(
    _dataset: Dataset,
    _metric: Metric,
    _config: NswConfig,
) -> vector_core_starter::Result<NswIndex> {
    todo!("Chapter 6: build NSW with the benchmark configuration")
}

fn build_hnsw(
    _dataset: Dataset,
    _metric: Metric,
    _config: HnswConfig,
) -> vector_core_starter::Result<HnswIndex> {
    todo!("Chapter 6: build HNSW with the benchmark configuration")
}

fn build_ivf_pq(
    _dataset: Dataset,
    _metric: Metric,
    _config: IvfPqConfig,
) -> vector_core_starter::Result<IvfPqIndex> {
    todo!("Chapter 6: build IVF-PQ with the benchmark configuration")
}

fn warm_up(indexes: &[BuiltIndex], _workload: &Workload) -> vector_core_starter::Result<()> {
    assert_eq!(indexes.len(), INDEX_COUNT);
    let _first_kind = indexes[0].index.kind();
    todo!("Chapter 6: run the balanced cyclic warm-up pass")
}

fn measure(
    _indexes: &[BuiltIndex],
    _workload: &Workload,
) -> vector_core_starter::Result<Vec<TimedRun>> {
    todo!("Chapter 6: time one balanced cyclic search pass")
}

fn summarize(run: &TimedRun, ground_truth: &[Vec<Neighbor>], k: usize) -> Measurement {
    assert_eq!(run.results.len(), ground_truth.len());
    assert_eq!(run.latencies.len(), ground_truth.len());
    let recall = run
        .results
        .iter()
        .zip(ground_truth)
        .map(|(actual, expected)| recall_at_k(expected, actual, k))
        .sum::<f64>()
        / ground_truth.len() as f64;
    assert!(recall.is_finite());
    assert!((0.0..=1.0).contains(&recall));

    let mut latencies = run.latencies.clone();
    latencies.sort_unstable();
    let p50 = percentile(&latencies, 50);
    let p99 = percentile(&latencies, 99);
    assert!(p99 >= p50);
    Measurement { recall, p50, p99 }
}

fn format_report(
    workload: &Workload,
    indexes: &[BuiltIndex],
    timed_runs: &[TimedRun],
    ground_truth: &[Vec<Neighbor>],
    accounting: PqAccounting,
) -> Vec<String> {
    assert_eq!(indexes.len(), INDEX_COUNT);
    assert_eq!(timed_runs.len(), INDEX_COUNT);
    assert_eq!(ground_truth.len(), workload.queries.len());

    let mut lines = indexes
        .iter()
        .zip(timed_runs)
        .enumerate()
        .map(|(ordinal, (index, run))| {
            assert_eq!(index.name, INDEX_NAMES[ordinal]);
            assert_eq!(index.index.kind(), index.name);
            assert_eq!(index.index.metric(), workload.metric);
            assert_eq!(index.index.dataset().vectors(), workload.dataset.vectors());
            let measurement = summarize(run, ground_truth, workload.k);
            if ordinal == 0 {
                assert_eq!(measurement.recall, 1.0);
            }
            format_row(index.name, workload, index.build_time, measurement)
        })
        .collect::<Vec<_>>();
    lines.push(format_accounting(accounting));
    lines
}

fn format_row(
    name: &str,
    workload: &Workload,
    build_time: Duration,
    measurement: Measurement,
) -> String {
    format!(
        "{name}: rows={}, dimensions={}, queries={}, metric=euclidean, k={}, build_ms={:.2}, recall={:.3}, p50_us={:.1}, p99_us={:.1}",
        workload.dataset.len(),
        workload.dataset.dimension(),
        workload.queries.len(),
        workload.k,
        build_time.as_secs_f64() * 1_000.0,
        measurement.recall,
        measurement.p50.as_secs_f64() * 1_000_000.0,
        measurement.p99.as_secs_f64() * 1_000_000.0,
    )
}

fn format_accounting(accounting: PqAccounting) -> String {
    format!(
        "ivf_pq search representation: codes_bytes={}, codebooks_bytes={}, search_bytes={}, full_vectors_bytes={}, compression={:.1}x",
        accounting.codes_bytes,
        accounting.codebooks_bytes,
        accounting.search_bytes(),
        accounting.full_vectors_bytes,
        accounting.compression(),
    )
}

fn percentile(_sorted: &[Duration], _percent: usize) -> Duration {
    todo!("Chapter 6: select a nearest-rank latency percentile")
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
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use super::*;

    fn assert_build_boundary(
        source: &str,
        elapsed_marker: &str,
        constructor_marker: &str,
        config_marker: Option<&str>,
        uses_try_new: bool,
    ) {
        let end = source.find(elapsed_marker).unwrap();
        let prefix = &source[..end];
        let constructor = prefix.rfind(constructor_marker).unwrap();
        let timer = prefix[..constructor]
            .rfind("let started = Instant::now();")
            .unwrap();
        let dataset = prefix[..timer]
            .rfind("let dataset = workload.dataset.clone();")
            .unwrap();
        let metric = prefix[..timer]
            .rfind("let metric = workload.metric;")
            .unwrap();
        assert!(dataset < metric);
        if let Some(config_marker) = config_marker {
            let config = prefix[..timer].rfind(config_marker).unwrap();
            assert!(metric < config);
        }

        let timed = &source[timer..end];
        assert_eq!(timed.matches("let ").count(), 2);
        assert_eq!(timed.matches("try_new(").count(), usize::from(uses_try_new));
        assert!(timed.contains(constructor_marker));
        assert!(!timed.contains("workload.metric"));
        assert!(!timed.contains("_config()"));
    }

    #[test]
    fn build_timers_include_only_constructor_work() {
        let source = include_str!("recall.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert_build_boundary(
            source,
            "let flat_build = started.elapsed();",
            "FlatIndex::try_new(dataset, metric)",
            None,
            true,
        );
        assert_build_boundary(
            source,
            "let ivf_flat_build = started.elapsed();",
            "IvfFlatIndex::try_new(dataset, metric, config)",
            Some("let config = ivf_flat_config();"),
            true,
        );
        assert_build_boundary(
            source,
            "let nsw_build = started.elapsed();",
            "build_nsw(dataset, metric, config)",
            Some("let config = nsw_config();"),
            false,
        );
        assert_build_boundary(
            source,
            "let hnsw_build = started.elapsed();",
            "build_hnsw(dataset, metric, config)",
            Some("let config = hnsw_config();"),
            false,
        );
        assert_build_boundary(
            source,
            "let ivf_pq_build = started.elapsed();",
            "build_ivf_pq(dataset, metric, config)",
            Some("let config = ivf_pq_config();"),
            false,
        );
    }

    #[test]
    fn contract_source_rejects_known_shortcuts() {
        let source = include_str!("recall.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let summarize = source
            .split("fn summarize(")
            .nth(1)
            .unwrap()
            .split("fn format_report(")
            .next()
            .unwrap();
        assert!(summarize.contains(".zip(ground_truth)"));
        assert!(summarize.contains("/ ground_truth.len() as f64"));
        assert!(!summarize.contains("ground_truth[0]"));

        let report = source
            .split("fn format_report(")
            .nth(1)
            .unwrap()
            .split("fn format_row(")
            .next()
            .unwrap();
        assert!(!report.contains(".take(4)"));
        assert!(report.contains("lines.push(format_accounting(accounting));"));

        let percentile = source
            .split("fn percentile(")
            .nth(1)
            .unwrap()
            .split("fn sample(")
            .next()
            .unwrap();
        assert!(!percentile.contains(".len()) / 100"));
        assert!(!percentile.contains(".len() - 1) / 100"));
    }

    #[test]
    fn inventory_and_configs_match_the_frozen_matrix() {
        assert_eq!(INDEX_NAMES, ["flat", "ivf_flat", "nsw", "hnsw", "ivf_pq"]);

        let ivf_flat = ivf_flat_config();
        assert_eq!(
            (
                ivf_flat.partitions,
                ivf_flat.probes,
                ivf_flat.iterations,
                ivf_flat.seed,
            ),
            (32, 6, 12, 7)
        );
        let nsw = nsw_config();
        assert_eq!(
            (nsw.max_connections, nsw.ef_construction, nsw.ef_search,),
            (12, 64, 40)
        );
        let hnsw = hnsw_config();
        assert_eq!(
            (
                hnsw.max_connections,
                hnsw.ef_construction,
                hnsw.ef_search,
                hnsw.max_level,
                hnsw.seed,
            ),
            (12, 64, 40, 12, 7)
        );
        let ivf_pq = ivf_pq_config();
        assert_eq!(
            (
                ivf_pq.partitions,
                ivf_pq.probes,
                ivf_pq.iterations,
                ivf_pq.subquantizers,
                ivf_pq.codebook_size,
                ivf_pq.rerank,
                ivf_pq.seed,
            ),
            (32, 6, 12, 4, 16, 100, 7)
        );
    }

    #[test]
    fn query_domains_pin_stride_offset_uniqueness_and_disjointness() {
        assert_eq!(QUERY_STRIDE, 17);
        assert_eq!(QUERY_OFFSET, 3);
        let dataset_domains = (0..ROWS).collect::<HashSet<_>>();
        let query_domains = (0..QUERIES)
            .map(|query| ROWS + query * QUERY_STRIDE + QUERY_OFFSET)
            .collect::<HashSet<_>>();
        assert_eq!(query_domains.len(), QUERIES);
        assert!(dataset_domains.is_disjoint(&query_domains));
    }

    #[test]
    fn workload_is_shared_deterministic_and_disjoint() {
        let left = Workload::fixed().unwrap();
        let right = Workload::fixed().unwrap();

        assert_eq!(left.dataset.vectors(), right.dataset.vectors());
        assert_eq!(left.queries, right.queries);
        assert_eq!(left.dataset.len(), ROWS);
        assert_eq!(left.dataset.dimension(), DIMENSIONS);
        assert_eq!(left.queries.len(), QUERIES);
        assert_eq!(left.metric, Metric::Euclidean);
        assert_eq!(left.k, K);
        for row in 0..ROWS {
            for dimension in 0..DIMENSIONS {
                assert_eq!(
                    left.dataset.vector(row)[dimension],
                    sample(row as u64, dimension as u64)
                );
            }
        }
        for (query, vector) in left.queries.iter().enumerate() {
            let domain = ROWS + query * QUERY_STRIDE + QUERY_OFFSET;
            assert!(domain >= ROWS);
            for dimension in 0..DIMENSIONS {
                assert_eq!(vector[dimension], sample(domain as u64, dimension as u64));
            }
        }
    }

    #[derive(Debug)]
    struct RecordingIndex {
        name: &'static str,
        dataset: Dataset,
        trace: Arc<Mutex<Vec<(&'static str, usize)>>>,
    }

    impl VectorIndex for RecordingIndex {
        fn kind(&self) -> &'static str {
            self.name
        }

        fn dataset(&self) -> &Dataset {
            &self.dataset
        }

        fn metric(&self) -> Metric {
            Metric::Euclidean
        }

        fn search(&self, query: &[f32], _k: usize) -> vector_core_starter::Result<Vec<Neighbor>> {
            self.trace
                .lock()
                .unwrap()
                .push((self.name, query[0] as usize));
            Ok(vec![Neighbor {
                row: query[0] as usize,
                distance: 0.0,
            }])
        }
    }

    fn recording_fixture() -> (
        Workload,
        Vec<BuiltIndex>,
        Arc<Mutex<Vec<(&'static str, usize)>>>,
    ) {
        let dataset = Dataset::try_new(vec![vec![0.0]]).unwrap();
        let queries = (0..QUERIES)
            .map(|query| vec![query as f32])
            .collect::<Vec<_>>();
        let workload = Workload {
            dataset: dataset.clone(),
            queries,
            metric: Metric::Euclidean,
            k: 1,
        };
        let trace = Arc::new(Mutex::new(Vec::new()));
        let indexes = INDEX_NAMES
            .iter()
            .map(|name| BuiltIndex {
                name,
                index: Box::new(RecordingIndex {
                    name,
                    dataset: dataset.clone(),
                    trace: Arc::clone(&trace),
                }),
                build_time: Duration::ZERO,
            })
            .collect();
        (workload, indexes, trace)
    }

    fn assert_balanced_trace(trace: &[(&'static str, usize)]) {
        assert_eq!(trace.len(), QUERIES * INDEX_COUNT);
        let mut position_counts = [[0_usize; INDEX_COUNT]; INDEX_COUNT];
        for (query, calls) in trace.chunks_exact(INDEX_COUNT).enumerate() {
            for (position, (name, observed_query)) in calls.iter().enumerate() {
                let expected_index = (query + position) % INDEX_COUNT;
                assert_eq!(*name, INDEX_NAMES[expected_index]);
                assert_eq!(*observed_query, query);
                position_counts[expected_index][position] += 1;
            }
        }
        assert!(
            position_counts
                .iter()
                .flatten()
                .all(|count| *count == QUERIES / INDEX_COUNT)
        );
    }

    #[test]
    fn warm_and_timed_phases_use_the_balanced_cyclic_trace() {
        let (workload, indexes, trace) = recording_fixture();

        warm_up(&indexes, &workload).unwrap();
        assert_balanced_trace(&trace.lock().unwrap());
        trace.lock().unwrap().clear();

        let runs = measure(&indexes, &workload).unwrap();
        assert_balanced_trace(&trace.lock().unwrap());
        assert!(
            runs.iter()
                .all(|run| run.latencies.len() == QUERIES && run.results.len() == QUERIES)
        );
    }

    #[test]
    fn nearest_rank_percentile_selects_expected_samples() {
        let samples = (1..=100).map(Duration::from_micros).collect::<Vec<_>>();
        let six_samples = (1..=6).map(Duration::from_micros).collect::<Vec<_>>();

        assert_eq!(percentile(&samples, 0), Duration::from_micros(1));
        assert_eq!(percentile(&samples, 50), Duration::from_micros(50));
        assert_eq!(percentile(&samples, 99), Duration::from_micros(99));
        assert_eq!(percentile(&samples, 100), Duration::from_micros(100));
        assert_eq!(percentile(&six_samples, 34), Duration::from_micros(3));
    }

    fn neighbors(rows: &[usize]) -> Vec<Neighbor> {
        rows.iter()
            .map(|row| Neighbor {
                row: *row,
                distance: *row as f32,
            })
            .collect()
    }

    #[test]
    fn summarize_uses_arithmetic_mean_recall() {
        let ground_truth = vec![neighbors(&[1, 2]), neighbors(&[3, 4]), neighbors(&[5, 6])];
        let run = TimedRun {
            latencies: vec![
                Duration::from_micros(2),
                Duration::from_micros(5),
                Duration::from_micros(8),
            ],
            results: vec![neighbors(&[1, 2]), neighbors(&[3, 9]), neighbors(&[7, 8])],
        };

        let measurement = summarize(&run, &ground_truth, 2);
        assert_eq!(measurement.recall, 0.5);
        assert!(measurement.recall.is_finite());
        assert!((0.0..=1.0).contains(&measurement.recall));
        assert!(measurement.p99 >= measurement.p50);
    }

    #[test]
    fn formatting_has_five_shared_rows_and_one_accounting_line() {
        let (workload, indexes, _) = recording_fixture();
        let ground_truth = workload
            .queries
            .iter()
            .map(|query| {
                vec![Neighbor {
                    row: query[0] as usize,
                    distance: 0.0,
                }]
            })
            .collect::<Vec<_>>();
        let timed_runs = (0..INDEX_COUNT)
            .map(|_| TimedRun {
                latencies: vec![Duration::from_micros(12); QUERIES],
                results: ground_truth.clone(),
            })
            .collect::<Vec<_>>();
        let accounting = PqAccounting {
            codes_bytes: 8_000,
            codebooks_bytes: 1_024,
            full_vectors_bytes: 128_000,
        };
        let lines = format_report(&workload, &indexes, &timed_runs, &ground_truth, accounting);

        assert_eq!(lines.len(), INDEX_COUNT + 1);
        for (ordinal, row) in lines[..INDEX_COUNT].iter().enumerate() {
            assert_eq!(
                row,
                &format!(
                    "{}: rows=1, dimensions=1, queries=100, metric=euclidean, k=1, build_ms=0.00, recall=1.000, p50_us=12.0, p99_us=12.0",
                    INDEX_NAMES[ordinal]
                )
            );
        }
        assert_eq!(
            lines[INDEX_COUNT],
            "ivf_pq search representation: codes_bytes=8000, codebooks_bytes=1024, search_bytes=9024, full_vectors_bytes=128000, compression=14.2x"
        );
    }

    #[test]
    fn ivf_pq_accounting_matches_the_frozen_workload() {
        let workload = Workload::fixed().unwrap();
        let index =
            IvfPqIndex::try_new(workload.dataset, Metric::Euclidean, ivf_pq_config()).unwrap();
        let accounting = PqAccounting::from_index(&index);

        assert_eq!(accounting.codes_bytes, 8_000);
        assert_eq!(accounting.codebooks_bytes, 1_024);
        assert_eq!(accounting.search_bytes(), 9_024);
        assert_eq!(accounting.full_vectors_bytes, 128_000);
        assert_eq!(format!("{:.1}", accounting.compression()), "14.2");
    }
}
