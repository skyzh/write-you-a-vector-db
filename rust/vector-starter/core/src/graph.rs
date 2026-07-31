use crate::{Dataset, Metric, Neighbor};

pub(crate) fn search_layer(
    _dataset: &Dataset,
    _metric: Metric,
    _query: &[f32],
    _adjacency: &[Vec<usize>],
    _entry_points: &[usize],
    _ef: usize,
    _allowed_rows: usize,
) -> Vec<Neighbor> {
    todo!("Chapter 3: traverse the graph with separate candidate and result frontiers")
}

pub(crate) fn prune_neighbors(
    _dataset: &Dataset,
    _metric: Metric,
    _owner: usize,
    _neighbors: &mut Vec<usize>,
    _max_connections: usize,
) {
    todo!("Chapter 3: retain the closest deterministic neighbor set")
}
