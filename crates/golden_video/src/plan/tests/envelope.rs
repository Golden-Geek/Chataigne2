use super::*;
use golden_time::{MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct Clock(AtomicU64);
impl golden_time::Clock for Clock {
    fn now(&self) -> Result<MonotonicTime, golden_time::TimeError> {
        Ok(MonotonicTime::from_nanos(self.0.load(Ordering::Relaxed)))
    }
}

#[test]
fn shared_time_envelopes_clamp_interpolate_reverse_settle_and_survive_show_discontinuities() {
    let clock = Arc::new(Clock::default());
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let anchor = authority.sample().unwrap();
    let later = anchor.at_monotonic(MonotonicTime::from_nanos(50)).unwrap();
    let future = OpacityEnvelope::new(
        later,
        Duration::from_nanos(100),
        Opacity::TRANSPARENT,
        Opacity::OPAQUE,
        InterpolationCurve::Linear,
    )
    .unwrap();
    assert_eq!(future.sample(anchor), Opacity::TRANSPARENT);
    let ramp = OpacityEnvelope::new(
        anchor,
        Duration::from_nanos(100),
        Opacity::TRANSPARENT,
        Opacity::OPAQUE,
        InterpolationCurve::Linear,
    )
    .unwrap();
    for (nanos, expected) in [(0, 0.0), (25, 0.25), (50, 0.5), (100, 1.0), (200, 1.0)] {
        clock.0.store(nanos, Ordering::Relaxed);
        assert_eq!(ramp.sample(authority.sample().unwrap()).get(), expected);
        if nanos == 50 {
            let paused = authority
                .discontinuity(ShowPosition::from_nanos(-10), PlaybackRate::PAUSED)
                .unwrap();
            assert_eq!(ramp.sample(paused).get(), 0.5);
        }
    }
    let mut other = TimeAuthority::new(Clock::default(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    assert_eq!(ramp.sample(other.sample().unwrap()), Opacity::OPAQUE);
    let smooth = OpacityEnvelope::new(
        anchor,
        Duration::from_nanos(100),
        Opacity::OPAQUE,
        Opacity::TRANSPARENT,
        InterpolationCurve::SmoothStep,
    )
    .unwrap();
    let quarter = anchor.at_monotonic(MonotonicTime::from_nanos(25)).unwrap();
    assert_eq!(smooth.sample(quarter).get(), 0.84375);
    let paused = authority
        .discontinuity(ShowPosition::from_nanos(-10), PlaybackRate::PAUSED)
        .unwrap();
    assert!(ramp.finished(paused));
    assert_eq!(ramp.sample(paused), Opacity::OPAQUE);
    let zero = OpacityEnvelope::new(
        anchor,
        Duration::ZERO,
        Opacity::TRANSPARENT,
        Opacity::OPAQUE,
        InterpolationCurve::Linear,
    )
    .unwrap();
    assert_eq!(zero.sample(anchor), Opacity::OPAQUE);
    assert!(
        OpacityEnvelope::new(
            anchor,
            Duration::MAX,
            Opacity::TRANSPARENT,
            Opacity::OPAQUE,
            InterpolationCurve::Linear
        )
        .is_err()
    );
}
