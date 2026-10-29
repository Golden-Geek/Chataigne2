use std::time::{Duration, Instant};

use windows::core::HRESULT;

use super::super::recovery::Recovery;
use crate::native::OutputState;

#[test]
fn retries_are_capped_and_constructor_success_does_not_hide_repeat_delivery_failure() {
    let mut recovery = Recovery::new();
    let mut now = Instant::now();
    let error = HRESULT(0x80004005_u32 as i32);
    for (index, delay) in [250, 500, 1000, 2000, 2000].into_iter().enumerate() {
        assert!(recovery.begin_attempt(now));
        recovery.created();
        recovery.failed(error, now);
        let deadline = now + Duration::from_millis(delay);
        assert!(!recovery.begin_attempt(deadline - Duration::from_nanos(1)));
        assert_eq!(recovery.attempts, index as u64 + 1);
        assert_eq!(recovery.state, OutputState::Recovering);
        now = deadline;
    }
    assert!(recovery.begin_attempt(now));
    recovery.created();
    recovery.accepted();
    recovery.failed(error, now);
    assert!(!recovery.begin_attempt(now + Duration::from_millis(249)));
    assert!(recovery.begin_attempt(now + Duration::from_millis(250)));
    recovery.stop();
    assert!(!recovery.begin_attempt(now + Duration::from_secs(10)));
    recovery.device_lost(error);
    assert!(!recovery.begin_attempt(now + Duration::from_secs(20)));
}
