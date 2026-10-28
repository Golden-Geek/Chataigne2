use std::time::Duration;

use crate::{EngineSamplePosition, MonotonicTime, ShowPosition, TimeError};

#[test]
fn coordinates_preserve_their_explicit_units_and_signed_show_positions() {
    assert_eq!(MonotonicTime::from_nanos(u64::MAX).as_nanos(), u64::MAX);
    assert_eq!(ShowPosition::from_nanos(i64::MIN).as_nanos(), i64::MIN);
    assert_eq!(ShowPosition::from_nanos(i64::MAX).as_nanos(), i64::MAX);
    assert_eq!(EngineSamplePosition::from_samples(u64::MAX).as_samples(), u64::MAX);
}

#[test]
fn monotonic_deadlines_do_not_wrap_or_saturate() {
    let maximum = MonotonicTime::from_nanos(u64::MAX);
    assert_eq!(maximum.checked_add(Duration::ZERO), Ok(maximum));
    assert_eq!(maximum.checked_add(Duration::from_nanos(1)), Err(TimeError::Overflow));
    assert_eq!(
        MonotonicTime::from_nanos(0).checked_add(Duration::from_secs(u64::MAX)),
        Err(TimeError::Overflow)
    );
    assert_eq!(
        MonotonicTime::from_nanos(10).checked_add(Duration::from_nanos(15)),
        Ok(MonotonicTime::from_nanos(25))
    );
}

#[test]
fn elapsed_time_rejects_a_coordinate_before_its_anchor() {
    let anchor = MonotonicTime::from_nanos(10);
    assert_eq!(anchor.duration_since(anchor), Ok(Duration::ZERO));
    assert_eq!(
        MonotonicTime::from_nanos(25).duration_since(anchor),
        Ok(Duration::from_nanos(15))
    );
    assert_eq!(
        MonotonicTime::from_nanos(9).duration_since(anchor),
        Err(TimeError::BeforeAnchor)
    );
}
