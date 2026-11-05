use super::next_resource_deadline;
use std::time::{Duration, Instant};

#[test]
fn resource_observations_keep_one_hz_phase_across_a_full_sample() {
    let origin = Instant::now();
    let mut deadline = origin;
    let mut observations = 0;
    // Service jitter previously accumulated into fewer than 119 observations.
    for millis in (0..120_000).step_by(33) {
        let now = origin + Duration::from_millis(millis);
        if now >= deadline {
            observations += 1;
            deadline = next_resource_deadline(deadline, now);
        }
    }
    assert_eq!(observations, 120);
    assert_eq!(deadline, origin + Duration::from_secs(120));
}

#[test]
fn delayed_resource_observation_skips_missed_slots_without_catch_up() {
    let origin = Instant::now();
    let observed = origin + Duration::from_millis(5_120);
    let next = next_resource_deadline(origin, observed);
    assert_eq!(next, origin + Duration::from_secs(6));
    assert!(next > observed);
}
