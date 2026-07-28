use vector_core::{
    Dataset, FlatIndex, IvfFlatConfig, IvfFlatIndex, Metric, Neighbor, NswConfig, NswIndex,
    VectorIndex, recall_at_k,
};

fn line_dataset(size: usize) -> Dataset {
    Dataset::try_new(
        (0..size)
            .map(|value| vec![value as f32, (value % 7) as f32 + 1.0])
            .collect(),
    )
    .unwrap()
}

#[test]
fn flat_search_is_deterministic_and_validates_queries() {
    let dataset = Dataset::try_new(vec![vec![1.0, 0.0], vec![-1.0, 0.0], vec![0.0, 1.0]]).unwrap();
    let index = FlatIndex::try_new(dataset, Metric::Euclidean).unwrap();

    let result = index.search(&[0.0, 0.0], 2).unwrap();
    assert_eq!(
        result.iter().map(|item| item.row).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(index.search(&[0.0], 1).is_err());
    assert!(index.search(&[f32::NAN, 0.0], 1).is_err());
}

#[test]
fn cosine_rejects_zero_norm_vectors() {
    let dataset = Dataset::try_new(vec![vec![1.0, 0.0], vec![0.0, 0.0]]).unwrap();
    assert!(FlatIndex::try_new(dataset, Metric::Cosine).is_err());
}

#[test]
fn recall_reports_result_overlap() {
    let expected = (0..4)
        .map(|row| Neighbor {
            row,
            distance: row as f32,
        })
        .collect::<Vec<_>>();
    let actual = [
        expected[0],
        expected[2],
        Neighbor {
            row: 9,
            distance: 3.0,
        },
    ];
    assert_eq!(recall_at_k(&expected, &actual, 3), 2.0 / 3.0);
}

#[test]
fn ivf_scanning_every_partition_matches_exact_search() {
    let dataset = line_dataset(80);
    let exact = FlatIndex::try_new(dataset.clone(), Metric::Euclidean).unwrap();
    let ivf = IvfFlatIndex::try_new(
        dataset,
        Metric::Euclidean,
        IvfFlatConfig {
            partitions: 8,
            probes: 2,
            iterations: 10,
            seed: 7,
        },
    )
    .unwrap();
    let query = [31.2, 4.0];

    let expected = exact.search(&query, 12).unwrap();
    let actual = ivf.search_with_probes(&query, 12, 8).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(ivf.list_sizes().iter().sum::<usize>(), 80);
}

#[test]
fn ivf_build_is_seeded() {
    let config = IvfFlatConfig {
        partitions: 6,
        probes: 2,
        iterations: 8,
        seed: 42,
    };
    let left = IvfFlatIndex::try_new(line_dataset(60), Metric::Euclidean, config).unwrap();
    let right = IvfFlatIndex::try_new(line_dataset(60), Metric::Euclidean, config).unwrap();
    assert_eq!(left.centroids(), right.centroids());
    assert_eq!(left.list_sizes(), right.list_sizes());
}

#[test]
fn ivf_cosine_recovers_from_a_zero_mean_cluster() {
    let dataset = Dataset::try_new(vec![
        vec![1.0, 0.0],
        vec![-1.0, 0.0],
        vec![0.0, 1.0],
        vec![0.0, -1.0],
    ])
    .unwrap();
    let index = IvfFlatIndex::try_new(
        dataset,
        Metric::Cosine,
        IvfFlatConfig {
            partitions: 1,
            probes: 1,
            iterations: 3,
            seed: 1,
        },
    )
    .unwrap();
    assert!(index.centroids()[0].iter().all(|value| value.is_finite()));
    assert_eq!(index.search(&[1.0, 0.0], 4).unwrap().len(), 4);
}

#[test]
fn nsw_high_ef_matches_exact_search_on_connected_fixture() {
    let dataset = line_dataset(64);
    let exact = FlatIndex::try_new(dataset.clone(), Metric::Euclidean).unwrap();
    let nsw = NswIndex::try_new(
        dataset,
        Metric::Euclidean,
        NswConfig {
            max_connections: 8,
            ef_construction: 32,
            ef_search: 32,
        },
    )
    .unwrap();
    let query = [17.4, 4.0];
    let expected = exact.search(&query, 10).unwrap();
    let actual = nsw.search_with_ef(&query, 10, 64).unwrap();
    assert_eq!(recall_at_k(&expected, &actual, 10), 1.0);
    assert!(nsw.adjacency().iter().all(|neighbors| neighbors.len() <= 8));
}
