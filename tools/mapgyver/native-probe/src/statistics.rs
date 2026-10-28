//! Bounded distributions shared by renderer timing and independent-consumer measurements.

use serde::{Deserialize, Serialize};

pub const MAX_SAMPLES: usize = 216_001;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub samples: usize,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub maximum_ms: f64,
}

#[derive(Default)]
pub struct Series {
    samples: Vec<f64>,
    pub truncated: u32,
}

impl Series {
    pub fn push(&mut self, value: f64) {
        if self.samples.len() < MAX_SAMPLES {
            self.samples.push(value);
        } else {
            self.truncated += 1;
        }
    }

    pub fn distribution(&mut self) -> Option<Distribution> {
        if self.samples.is_empty() {
            return None;
        }
        self.samples.sort_by(f64::total_cmp);
        let count = self.samples.len();
        let rank = |percent: usize| self.samples[(count * percent).div_ceil(100) - 1];
        Some(Distribution {
            samples: count,
            mean_ms: self.samples.iter().sum::<f64>() / count as f64,
            median_ms: rank(50),
            p95_ms: rank(95),
            p99_ms: rank(99),
            maximum_ms: self.samples[count - 1],
        })
    }
}

#[cfg(test)]
mod tests;
