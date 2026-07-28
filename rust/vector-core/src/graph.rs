use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

use crate::search::TopK;
use crate::{Dataset, Metric, Neighbor};

pub(crate) fn search_layer(
    dataset: &Dataset,
    metric: Metric,
    query: &[f32],
    adjacency: &[Vec<usize>],
    entry_points: &[usize],
    ef: usize,
    allowed_rows: usize,
) -> Vec<Neighbor> {
    let ef = ef.max(1).min(allowed_rows);
    if allowed_rows == 0 || entry_points.is_empty() {
        return Vec::new();
    }

    let mut visited = HashSet::new();
    let mut candidates = BinaryHeap::new();
    let mut best = TopK::new(ef);
    for &entry in entry_points {
        if entry >= allowed_rows || !visited.insert(entry) {
            continue;
        }
        let neighbor = Neighbor {
            row: entry,
            distance: metric.distance(query, dataset.vector(entry)),
        };
        candidates.push(Reverse(neighbor));
        best.push(neighbor);
    }

    while let Some(Reverse(current)) = candidates.pop() {
        if best.len() >= ef && best.worst().is_some_and(|worst| current > worst) {
            break;
        }
        for &row in &adjacency[current.row] {
            if row >= allowed_rows || !visited.insert(row) {
                continue;
            }
            let neighbor = Neighbor {
                row,
                distance: metric.distance(query, dataset.vector(row)),
            };
            if best.len() < ef || best.worst().is_some_and(|worst| neighbor < worst) {
                candidates.push(Reverse(neighbor));
                best.push(neighbor);
            }
        }
    }

    best.into_sorted()
}

pub(crate) fn prune_neighbors(
    dataset: &Dataset,
    metric: Metric,
    owner: usize,
    neighbors: &mut Vec<usize>,
    max_connections: usize,
) {
    neighbors.sort_unstable();
    neighbors.dedup();
    neighbors.sort_unstable_by(|left, right| {
        let left = Neighbor {
            row: *left,
            distance: metric.distance(dataset.vector(owner), dataset.vector(*left)),
        };
        let right = Neighbor {
            row: *right,
            distance: metric.distance(dataset.vector(owner), dataset.vector(*right)),
        };
        left.cmp(&right)
    });
    neighbors.truncate(max_connections);
}
