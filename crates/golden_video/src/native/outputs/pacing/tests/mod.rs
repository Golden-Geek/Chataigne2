use super::*;
use golden_time::{Clock, MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority, TimeError};

#[derive(Clone)]
struct Fixed(std::sync::Arc<std::sync::atomic::AtomicU64>);
impl Clock for Fixed {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(MonotonicTime::from_nanos(
            self.0.load(std::sync::atomic::Ordering::Relaxed),
        ))
    }
}
#[test]
fn delivery_phase_survives_seek_and_resets_only_for_a_new_authority() {
    let clock = Fixed(Default::default());
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let mut pacing = Pacing::new(Some(RationalRate::new(30, 1).unwrap()));
    assert!(pacing.due(authority.sample().unwrap()));
    assert!(
        !pacing.ready(authority.sample().unwrap()),
        "no unused GPU batch while the consumer is not due"
    );
    assert!(!pacing.due(authority.sample().unwrap()));
    clock.0.store(10_000_000, std::sync::atomic::Ordering::Relaxed);
    let seek = authority
        .discontinuity(ShowPosition::from_nanos(900), PlaybackRate::PAUSED)
        .unwrap();
    assert!(!pacing.due(seek), "show transport cannot restart output cadence");
    clock.0.store(33_333_334, std::sync::atomic::Ordering::Relaxed);
    assert!(pacing.ready(authority.sample().unwrap()));
    assert!(pacing.due(authority.sample().unwrap()));
    clock.0.store(40_000_000, std::sync::atomic::Ordering::Relaxed);
    let mut replaced = TimeAuthority::new(clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    assert!(
        pacing.due(replaced.sample().unwrap()),
        "new authority starts even with an increased coordinate"
    );
}
