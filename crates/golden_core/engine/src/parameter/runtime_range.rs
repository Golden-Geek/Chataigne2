//! Shared availability is authoritative for reads/writes before its UI wake is delivered.
use super::RangeConstraint;
use std::sync::{Arc, Mutex};

/// Finite ordered scalar availability, independent of authored constraints and history.
/// Owners update this bounded cell, then wake their node to publish changed constraints.
#[derive(Clone, Debug)]
pub struct RuntimeScalarRange(Arc<Mutex<(f64, f64)>>);

impl RuntimeScalarRange {
    /// Create a shared range; rejects non-finite or reversed endpoints.
    pub fn new(min: f64, max: f64) -> Result<Self, String> {
        validate(min, max)?;
        Ok(Self(Arc::new(Mutex::new((min, max)))))
    }
    /// Replace both endpoints together, leaving the previous range intact on rejection.
    pub fn set(&self, min: f64, max: f64) -> Result<(), String> {
        validate(min, max)?;
        // The guard never escapes this module; assignment cannot leave partial bounds.
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = (min, max);
        Ok(())
    }
    /// Read one coherent range for normalization or snapshot projection.
    pub fn range(&self) -> RangeConstraint {
        let (min, max) = *self.0.lock().unwrap_or_else(|error| error.into_inner());
        RangeConstraint::Uniform {
            min: Some(min),
            max: Some(max),
        }
    }
}
fn validate(min: f64, max: f64) -> Result<(), String> {
    if !min.is_finite() || !max.is_finite() || min > max {
        return Err("runtime scalar bounds must be finite and ordered".into());
    }
    Ok(())
}
