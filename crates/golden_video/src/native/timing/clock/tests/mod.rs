use super::*;

#[test]
fn publication_projection_rounds_earlier_and_rejects_unrepresentable_clocks() {
    assert_eq!(earliest_qpc(100, Duration::ZERO, 10_000_000), Some(98));
    assert_eq!(earliest_qpc(100, Duration::from_nanos(101), 10_000_000), Some(97));
    assert_eq!(earliest_qpc(100, Duration::from_nanos(99), 10_000_000), Some(98));
    assert_eq!(earliest_qpc(100, Duration::from_secs(1), 0), None);
    assert_eq!(earliest_qpc(0, Duration::ZERO, 10_000_000), None);
    assert_eq!(earliest_qpc(u64::MAX, Duration::MAX, u64::MAX), None);
}

#[test]
fn windows_publication_bound_precedes_the_direct_after_stamp() {
    let clock = Clock::new();
    let published = Instant::now();
    let after = clock.stamp().expect("Windows QPC");
    let bound = clock.lower_bound(published).unwrap();
    assert!(bound <= after);
    assert!(clock.lower_bound(Instant::now() + Duration::from_secs(1)).is_none());
}
