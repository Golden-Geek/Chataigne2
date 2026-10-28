use super::{Cadence, Submission};
use std::time::{Duration, Instant};

#[test]
fn busy_and_hidden_outputs_do_not_become_accepted_frames_and_warmup_is_excluded() {
    let start = Instant::now();
    let mut cadence = Cadence::default();
    cadence.record(start, start, false, Submission::Accepted);
    for (offset, outcome) in [
        (10, Submission::Accepted),
        (20, Submission::Busy),
        (30, Submission::Occluded),
        (40, Submission::NoTarget),
        (50, Submission::Accepted),
    ] {
        cadence.record(
            start + Duration::from_millis(offset),
            start + Duration::from_millis(offset - 1),
            true,
            outcome,
        );
    }
    let report = cadence.finish();
    assert_eq!(report.attempts, 5);
    assert_eq!(
        (report.accepted_calls, report.busy, report.occluded, report.no_target),
        (2, 1, 1, 1)
    );
    assert_eq!(report.late_attempts, 5);
    let intervals = report.accepted_call_intervals.unwrap();
    assert_eq!(intervals.samples, 1);
    assert_eq!(intervals.maximum_ms, 40.0);
    assert_eq!(report.deadline_lateness.unwrap().p95_ms, 1.0);
}
