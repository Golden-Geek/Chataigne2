use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use golden_time::{
    ActivationStamp, ActivationState, Clock, EngineSamplePosition, MonotonicTime, PlaybackRate, RationalRate, Rounding,
    SampleMapping, ShowPosition, TimeAuthority, TimeError,
};

struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(MonotonicTime::from_nanos(self.0.load(Ordering::Acquire)))
    }
}

#[test]
fn independent_scheduled_consumers_share_one_epoch_and_explicit_sample_time_conversion() {
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let source: Arc<dyn Clock> = clock.clone();
    let mut authority = TimeAuthority::new(source, ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let initial = authority.sample().unwrap();
    let samples = SampleMapping::new(
        initial.monotonic(),
        EngineSamplePosition::from_samples(0),
        RationalRate::new(48_000, 1).unwrap(),
    );
    let at = samples
        .monotonic_at(EngineSamplePosition::from_samples(48_000), Rounding::Ceil)
        .unwrap();
    let first = ActivationStamp::new(initial.epoch(), at);
    let second = ActivationStamp::new(initial.epoch(), at);
    let control = ActivationStamp::new(initial.epoch(), at);
    assert_eq!(
        first.check(initial),
        Ok(ActivationState::Pending {
            remaining: Duration::from_secs(1)
        })
    );

    clock.0.store(1_000_000_000, Ordering::Release);
    for stamp in [first, second, control] {
        let now = authority.sample().unwrap();
        assert_eq!(
            stamp.check(now),
            Ok(ActivationState::Due {
                lateness: Duration::ZERO
            })
        );
        assert_eq!(now.show_position().as_nanos(), 1_000_000_000);
        assert_eq!(
            samples
                .sample_at(now.monotonic(), Rounding::Floor)
                .unwrap()
                .as_samples(),
            48_000
        );
    }
    let changed = authority
        .discontinuity(ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap();
    assert_eq!(first.check(changed), Err(TimeError::StaleEpoch));
    assert_eq!(
        samples
            .sample_at(changed.monotonic(), Rounding::Floor)
            .unwrap()
            .as_samples(),
        48_000
    );
}
