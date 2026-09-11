use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One recorded sample used when fitting a curve to bezier keys.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CurveFitPoint {
    /// Sample position on the curve domain axis.
    pub position: f64,
    /// Sample value on the curve value axis.
    pub value: f64,
}

impl CurveFitPoint {
    /// Creates one fit sample from `position` and `value`.
    pub fn new(position: f64, value: f64) -> Self {
        Self { position, value }
    }
}

/// Tuning parameters for sample-to-bezier fitting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CurveBezierFitOptions {
    /// Maximum tolerated absolute value error between source samples and the fitted curve.
    #[serde(default = "default_curve_fit_max_value_error")]
    pub max_value_error: f64,
    /// Maximum number of keys emitted by the fit.
    #[serde(default = "default_curve_fit_max_keys")]
    pub max_keys: usize,
}

impl Default for CurveBezierFitOptions {
    fn default() -> Self {
        Self {
            max_value_error: default_curve_fit_max_value_error(),
            max_keys: default_curve_fit_max_keys(),
        }
    }
}

fn default_curve_fit_max_value_error() -> f64 {
    0.01
}

fn default_curve_fit_max_keys() -> usize {
    12
}
