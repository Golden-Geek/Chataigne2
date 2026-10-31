//! Fixed-size interpolation evaluated from the render worker's shared monotonic observation.
use crate::Opacity;
use golden_time::{TimeError, TimeSnapshot};
use std::time::Duration;

/// Shape of a normalized interpolation interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpolationCurve {
    /// Constant change per unit of elapsed time.
    Linear,
    /// Cubic interpolation with zero endpoint derivatives.
    SmoothStep,
}

/// Immutable, checked opacity factor. Evaluation allocates nothing and never advances a clock.
#[derive(Clone, Copy, Debug)]
pub struct OpacityEnvelope {
    anchor: TimeSnapshot,
    duration: Duration,
    from: Opacity,
    to: Opacity,
    curve: InterpolationCurve,
}
impl OpacityEnvelope {
    /// Constructs an interval, rejecting a deadline outside the authority's coordinate range.
    pub fn new(
        anchor: TimeSnapshot,
        duration: Duration,
        from: Opacity,
        to: Opacity,
        curve: InterpolationCurve,
    ) -> Result<Self, TimeError> {
        anchor.monotonic().checked_add(duration)?;
        Ok(Self {
            anchor,
            duration,
            from,
            to,
            curve,
        })
    }
    /// Evaluates the clamped interval. Replacing the monotonic authority settles to the target;
    /// show-position/rate discontinuities within that same authority do not restart the fade.
    pub fn sample(self, now: TimeSnapshot) -> Opacity {
        if self.finished(now) {
            return self.to;
        }
        let elapsed = now
            .monotonic()
            .as_nanos()
            .saturating_sub(self.anchor.monotonic().as_nanos());
        let t = elapsed as f64 / self.duration.as_nanos() as f64;
        let t = match self.curve {
            InterpolationCurve::Linear => t,
            InterpolationCurve::SmoothStep => t * t * (3.0 - 2.0 * t),
        };
        let value = f64::from(self.from.get()) + (f64::from(self.to.get()) - f64::from(self.from.get())) * t;
        Opacity::new((value as f32).clamp(0.0, 1.0)).expect("checked opacity interpolation")
    }
    /// Whether the target is reached, including zero duration or authority replacement.
    pub fn finished(self, now: TimeSnapshot) -> bool {
        !self.anchor.epoch().same_authority(now.epoch())
            || self.duration.is_zero()
            || now
                .monotonic()
                .as_nanos()
                .saturating_sub(self.anchor.monotonic().as_nanos()) as u128
                >= self.duration.as_nanos()
    }
    /// Exact target factor, useful when pruning completed inputs outside the frame loop.
    pub fn target(self) -> Opacity {
        self.to
    }
}
