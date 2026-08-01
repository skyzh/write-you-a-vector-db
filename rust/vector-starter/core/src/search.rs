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

    pub(crate) fn worst(&self) -> Option<Neighbor> {
        self.heap.peek().copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.heap.len()
    }

    pub(crate) fn into_sorted(mut self) -> Vec<Neighbor> {
        let mut neighbors = self.heap.drain().collect::<Vec<_>>();
        neighbors.sort_unstable();
        neighbors
    }
}

pub fn recall_at_k(_expected: &[Neighbor], _actual: &[Neighbor], _k: usize) -> f64 {
    todo!("Chapter 2: compute top-k row-id overlap")
}

#[derive(Debug, Clone)]
pub(crate) struct DeterministicRng(u64);

impl DeterministicRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(crate) fn index(&mut self, upper_bound: usize) -> usize {
        debug_assert!(upper_bound > 0);
        (self.next_u64() % upper_bound as u64) as usize
    }
}
