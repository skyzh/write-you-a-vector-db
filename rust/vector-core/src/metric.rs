#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Euclidean,
    Cosine,
    Dot,
}

impl Metric {
    pub fn distance(self, left: &[f32], right: &[f32]) -> f32 {
        debug_assert_eq!(left.len(), right.len());
        match self {
            Self::Euclidean => left
                .iter()
                .zip(right)
                .map(|(left, right)| {
                    let delta = f64::from(*left) - f64::from(*right);
                    delta * delta
                })
                .sum::<f64>()
                .sqrt() as f32,
            Self::Cosine => {
                let dot = left
                    .iter()
                    .zip(right)
                    .map(|(left, right)| f64::from(*left) * f64::from(*right))
                    .sum::<f64>();
                let denominator =
                    Self::squared_norm(left).sqrt() * Self::squared_norm(right).sqrt();
                (1.0 - dot / denominator).clamp(0.0, 2.0) as f32
            }
            Self::Dot => -left
                .iter()
                .zip(right)
                .map(|(left, right)| f64::from(*left) * f64::from(*right))
                .sum::<f64>() as f32,
        }
    }

    pub(crate) fn squared_norm(vector: &[f32]) -> f64 {
        vector
            .iter()
            .map(|value| {
                let value = f64::from(*value);
                value * value
            })
            .sum()
    }
}
