use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy)]
pub struct Neighbor {
    pub row: usize,
    pub distance: f32,
}

impl PartialEq for Neighbor {
    fn eq(&self, other: &Self) -> bool {
        self.row == other.row && self.distance.total_cmp(&other.distance).is_eq()
    }
}

impl Eq for Neighbor {}

impl PartialOrd for Neighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Neighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.row.cmp(&other.row))
    }
}

#[derive(Debug)]
pub(crate) struct TopK {
    k: usize,
    heap: BinaryHeap<Neighbor>,
}

impl TopK {
    pub(crate) fn new(k: usize) -> Self {
        Self {
            k,
            heap: BinaryHeap::with_capacity(k.saturating_add(1)),
        }
    }

    pub(crate) fn push(&mut self, neighbor: Neighbor) {
        if self.k == 0 {
            return;
        }
        self.heap.push(neighbor);
        if self.heap.len() > self.k {
            self.heap.pop();
        }
    }

    pub(crate) fn into_sorted(mut self) -> Vec<Neighbor> {
        let mut neighbors = self.heap.drain().collect::<Vec<_>>();
        neighbors.sort_unstable();
        neighbors
    }
}
