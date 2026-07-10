use golden_values::FiniteF64;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTarget {
    pub id: u64,
    pub position: [FiniteF64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedTarget {
    pub id: u64,
    pub weight: FiniteF64,
}

pub struct Spatializer {
    targets: Vec<SpatialTarget>,
}

impl Spatializer {
    pub fn compile(mut targets: Vec<SpatialTarget>) -> Result<Self, SpatializerError> {
        if targets.is_empty() {
            return Err(SpatializerError::NoTargets);
        }
        targets.sort_by_key(|target| target.id);
        if targets.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(SpatializerError::DuplicateTarget);
        }
        Ok(Self { targets })
    }

    pub fn project(&self, source: [FiniteF64; 2]) -> Vec<WeightedTarget> {
        let mut nearest = self
            .targets
            .iter()
            .map(|target| {
                let dx = target.position[0].get() - source[0].get();
                let dy = target.position[1].get() - source[1].get();
                (target.id, dx * dx + dy * dy)
            })
            .collect::<Vec<_>>();
        nearest.sort_by(|left, right| left.1.total_cmp(&right.1).then(left.0.cmp(&right.0)));
        if nearest[0].1 == 0.0 {
            return vec![WeightedTarget {
                id: nearest[0].0,
                weight: FiniteF64::new(1.0).unwrap(),
            }];
        }
        nearest.truncate(3);
        let inverse_sum = nearest.iter().map(|(_, distance)| 1.0 / distance).sum::<f64>();
        nearest
            .into_iter()
            .map(|(id, distance)| WeightedTarget {
                id,
                weight: FiniteF64::new((1.0 / distance) / inverse_sum).unwrap(),
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SpatializerError {
    #[error("spatializer requires at least one target")]
    NoTargets,
    #[error("spatializer target identifiers must be unique")]
    DuplicateTarget,
}
