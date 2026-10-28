use super::*;

#[test]
fn a_busy_editor_does_not_consume_the_independent_outputs_reservation() {
    let (mut editor, mut output) = (Admission::default(), Admission::default());
    for _ in 0..100 {
        assert!(!editor.ready(|| Ok::<_, ()>(false)).unwrap());
        assert!(output.ready(|| Ok::<_, ()>(true)).unwrap());
        output.presented(Submission::Accepted);
    }
    assert_eq!(editor.report().not_ready, 100);
    assert_eq!(editor.report().accepted_presents, 0);
    assert_eq!(output.report().accepted_presents, 100);
}

#[test]
fn unsuccessful_present_keeps_its_reservation_without_repolling_a_consumed_signal() {
    let mut queue = Admission::default();
    assert!(queue.ready(|| Ok::<_, ()>(true)).unwrap());
    queue.presented(Submission::Busy);
    assert!(queue.ready(|| Err::<bool, _>("must not repoll")).unwrap());
    queue.presented(Submission::Occluded);
    assert!(queue.ready(|| Err::<bool, _>("must not repoll")).unwrap());
    queue.presented(Submission::Accepted);
    assert!(!queue.ready(|| Ok::<_, ()>(false)).unwrap());
    let report = queue.report();
    assert_eq!(
        (report.readiness_polls, report.ready_signals, report.not_ready),
        (2, 1, 1)
    );
    assert_eq!(
        (
            report.reservation_retries,
            report.busy_presents,
            report.occluded_presents
        ),
        (2, 1, 1)
    );
    assert_eq!(report.accepted_presents, 1);
}

#[test]
fn a_readiness_error_cannot_become_a_ready_or_timeout_observation() {
    let mut queue = Admission::default();
    assert_eq!(queue.ready(|| Err::<bool, _>("invalid handle")), Err("invalid handle"));
    assert_eq!(queue.report().ready_signals, 0);
    assert_eq!(queue.report().not_ready, 0);
    assert!(queue.ready(|| Ok::<_, ()>(true)).unwrap());
}
