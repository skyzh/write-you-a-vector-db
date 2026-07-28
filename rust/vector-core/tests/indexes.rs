use vector_core::{Dataset, FlatIndex, Metric, Neighbor, VectorIndex, recall_at_k};

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
