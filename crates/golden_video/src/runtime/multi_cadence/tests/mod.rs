use super::*;
use std::time::Duration;

#[test]
fn mixed_deadlines_include_exact_fifty_hz_and_fractional_display_slots() {
    let primary = RationalRate::new(60, 1).unwrap();
    let fifty = RationalRate::new(50, 1).unwrap();
    let fractional = RationalRate::new(60_000, 1_001).unwrap();
    let anchor = MonotonicTime::from_nanos(0);
    let mut clocks = Cadences::new(primary, &[fifty, fractional, primary], anchor);
    assert_eq!(clocks.clocks.len(), 3);
    assert!(clocks.consume_due(anchor).unwrap().is_some());
    assert_eq!(clocks.deadline().unwrap().as_nanos(), 16_666_667);
    clocks.consume_due(clocks.deadline().unwrap()).unwrap().unwrap();
    assert_eq!(clocks.deadline().unwrap().as_nanos(), 16_683_334);
    clocks.consume_due(clocks.deadline().unwrap()).unwrap().unwrap();
    assert_eq!(clocks.deadline().unwrap().as_nanos(), 20_000_000);
    // A long stall coalesces every clock rather than replaying all missed frames.
    assert!(
        clocks
            .consume_due(anchor.checked_add(Duration::from_secs(100)).unwrap())
            .unwrap()
            .is_some()
    );
    assert!(
        clocks
            .consume_due(anchor.checked_add(Duration::from_secs(100)).unwrap())
            .unwrap()
            .is_none()
    );
}
