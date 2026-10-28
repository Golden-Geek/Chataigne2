use super::*;

#[test]
fn blocked_gpu_coalesces_third_batch_and_only_completed_prefix_releases_capacity() {
    let mut admission = Admission::default();
    let first = admission.reserve(false, |_| Ok::<_, ()>(false)).unwrap().unwrap();
    let first_slot = first.slot();
    admission.submitted(first);
    let second = admission.reserve(true, |_| Ok::<_, ()>(false)).unwrap().unwrap();
    admission.submitted(second);
    let mut polls = Vec::new();
    for _ in 0..100 {
        assert!(
            admission
                .reserve(true, |slot| {
                    polls.push(slot);
                    Ok::<_, ()>(false)
                })
                .unwrap()
                .is_none()
        );
    }
    assert!(polls.iter().all(|slot| *slot == first_slot));
    assert_eq!(admission.report().sampled.busy, 100);
    assert_eq!(admission.report().peak_in_flight, 2);
    let next = admission
        .reserve(true, |slot| Ok::<_, ()>(slot == first_slot))
        .unwrap()
        .unwrap();
    assert_eq!(next.slot(), first_slot);
    admission.submitted(next);
    assert_eq!(admission.report().pending, 2);
    admission.poll(|_| Ok::<_, ()>(true)).unwrap();
    let report = admission.report();
    assert_eq!(report.completed, report.submitted);
    assert_eq!(report.pending, 0);
    assert_eq!(report.warmup.attempts, 1);
    assert_eq!(report.sampled.attempts, report.sampled.admitted + report.sampled.busy);
}

#[test]
fn polling_errors_never_become_a_free_slot_or_busy_sample() {
    let mut admission = Admission::default();
    let first = admission.reserve(true, |_| Ok::<_, &str>(true)).unwrap().unwrap();
    admission.submitted(first);
    assert_eq!(admission.reserve(true, |_| Err("device lost")), Err("device lost"));
    let report = admission.report();
    assert_eq!(report.sampled.attempts, 1);
    assert_eq!(report.completed, 0);
    assert_eq!(report.pending, 1);
    assert!(!report.reserved);
}
