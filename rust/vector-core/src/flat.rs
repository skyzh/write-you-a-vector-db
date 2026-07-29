use crate::search::TopK;
use crate::{Dataset, Metric, Neighbor, Result, VectorIndex};

#[derive(Debug, Clone)]
pub struct FlatIndex {
    dataset: Dataset,
    metric: Metric,
}

impl FlatIndex {
    pub fn try_new(dataset: Dataset, metric: Metric) -> Result<Self> {
        dataset.validate_for_metric(metric)?;
        Ok(Self { dataset, metric })
    }
}

impl VectorIndex for FlatIndex {
    fn kind(&self) -> &'static str {
        "flat"
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn metric(&self) -> Metric {
        self.metric
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<Neighbor>> {
        self.dataset.validate_query(query, self.metric)?;
        let mut result = TopK::new(k.min(self.dataset.len()));
        for (row, vector) in self.dataset.vectors().iter().enumerate() {
            result.push(Neighbor {
                row,
                distance: self.metric.distance(vector, query),
            });
        }
        Ok(result.into_sorted())
    }
}
