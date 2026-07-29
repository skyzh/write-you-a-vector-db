use vector_core_starter::{Dataset, FlatIndex, Metric, VectorIndex};

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
