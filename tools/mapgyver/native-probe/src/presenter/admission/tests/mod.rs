use super::*;

#[test]
fn a_busy_editor_does_not_consume_the_independent_outputs_reservation() {
    let (mut editor, mut output) = (Admission::default(), Admission::default());
    for _ in 0..100 {
        assert!(!editor.ready(true, || Ok::<_, ()>(false)).unwrap());
        assert!(output.ready(true, || Ok::<_, ()>(true)).unwrap());
        output.presented(true, Submission::Accepted);
    }
    assert_eq!(editor.report().all.not_ready, 100);
    assert_eq!(editor.report().sampled.not_ready, 100);
    assert_eq!(editor.report().all.accepted_presents, 0);
    assert_eq!(editor.report().sampled.accepted_presents, 0);
    assert_eq!(output.report().all.accepted_presents, 100);
    assert_eq!(output.report().sampled.accepted_presents, 100);
}

#[test]
fn unsuccessful_present_keeps_its_reservation_without_repolling_a_consumed_signal() {
    let mut queue = Admission::default();
    assert!(queue.ready(true, || Ok::<_, ()>(true)).unwrap());
    queue.presented(true, Submission::Busy);
    assert!(queue.ready(true, || Err::<bool, _>("must not repoll")).unwrap());
    queue.presented(true, Submission::Occluded);
    assert!(queue.ready(true, || Err::<bool, _>("must not repoll")).unwrap());
    queue.presented(true, Submission::Accepted);
    assert!(!queue.ready(true, || Ok::<_, ()>(false)).unwrap());
    let report = queue.report();
    for counts in [&report.all, &report.sampled] {
        assert_eq!(
            (counts.readiness_polls, counts.ready_signals, counts.not_ready),
            (2, 1, 1)
        );
        assert_eq!(
            (
                counts.reservation_retries,
                counts.busy_presents,
                counts.occluded_presents
            ),
            (2, 1, 1)
        );
        assert_eq!(counts.accepted_presents, 1);
    }
}

#[test]
fn a_readiness_error_cannot_become_a_ready_or_timeout_observation() {
    let mut queue = Admission::default();
    assert_eq!(
        queue.ready(true, || Err::<bool, _>("invalid handle")),
        Err("invalid handle")
    );
    let report = queue.report();
    for counts in [&report.all, &report.sampled] {
        assert_eq!(counts.readiness_polls, 1);
        assert_eq!(counts.ready_signals, 0);
        assert_eq!(counts.not_ready, 0);
        assert_eq!(counts.accepted_presents, 0);
    }
    assert!(!queue.ready(true, || Ok::<_, ()>(false)).unwrap());
    assert!(queue.ready(true, || Ok::<_, ()>(true)).unwrap());
}

#[test]
fn a_warmup_reservation_counts_retries_and_present_outcomes_in_the_actual_sample() {
    let mut queue = Admission::default();
    assert!(queue.ready(false, || Ok::<_, ()>(true)).unwrap());
    queue.presented(false, Submission::Busy);
    assert!(
        queue
            .ready(true, || Err::<bool, _>("warmup signal is already reserved"))
            .unwrap()
    );
    queue.presented(true, Submission::Busy);
    assert!(
        queue
            .ready(true, || Err::<bool, _>("must retain the same reservation"))
            .unwrap()
    );
    queue.presented(true, Submission::Accepted);
    assert!(!queue.ready(true, || Ok::<_, ()>(false)).unwrap());

    let report = queue.report();
    assert_eq!((report.all.readiness_polls, report.all.ready_signals), (2, 1));
    assert_eq!((report.sampled.readiness_polls, report.sampled.ready_signals), (1, 0));
    assert_eq!((report.all.busy_presents, report.sampled.busy_presents), (2, 1));
    assert_eq!((report.all.not_ready, report.sampled.not_ready), (1, 1));
    assert_eq!(
        (report.all.reservation_retries, report.sampled.reservation_retries),
        (2, 2)
    );
    assert_eq!((report.all.accepted_presents, report.sampled.accepted_presents), (1, 1));
}

#[test]
fn warmup_readiness_and_present_failures_do_not_become_sampled_busy_counts() {
    let mut queue = Admission::default();
    assert!(!queue.ready(false, || Ok::<_, ()>(false)).unwrap());
    assert!(queue.ready(false, || Ok::<_, ()>(true)).unwrap());
    queue.presented(false, Submission::Occluded);
    assert!(queue.ready(false, || Err::<bool, _>("must not repoll")).unwrap());
    queue.presented(false, Submission::Accepted);

    assert!(!queue.ready(true, || Ok::<_, ()>(false)).unwrap());
    assert!(queue.ready(true, || Ok::<_, ()>(true)).unwrap());
    queue.presented(true, Submission::Busy);
    assert!(queue.ready(true, || Err::<bool, _>("must not repoll")).unwrap());
    queue.presented(true, Submission::Accepted);

    let report = queue.report();
    assert_eq!((report.all.not_ready, report.sampled.not_ready), (2, 1));
    assert_eq!((report.all.occluded_presents, report.sampled.occluded_presents), (1, 0));
    assert_eq!((report.all.busy_presents, report.sampled.busy_presents), (1, 1));
    assert_eq!((report.all.accepted_presents, report.sampled.accepted_presents), (2, 1));
}
