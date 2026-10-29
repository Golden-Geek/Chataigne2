use super::*;

#[test]
fn a_successful_current_frame_restores_serving_after_a_pre_use_submission_failure() {
    let mut h = Harness::new(config(), NullConfig::default());
    let request = h.submit(plan(1, 1, 1));
    let (original, _) = h.installed(request);
    let before = h.runtime().snapshot().frames_submitted;
    h.null.fail_next_submit();
    let next = h.clock.0.load(Ordering::Acquire) + 20_000_000;
    h.advance(next);
    h.wait("pre-use submission failure", |h| {
        h.runtime().snapshot().state == RuntimeState::Faulted
    });
    assert_eq!(h.runtime().snapshot().failure.unwrap().code(), FailureCode::Backend);
    assert_eq!(h.runtime().snapshot().resources.unwrap().active.unwrap(), original);
    h.drive("next successful frame restores serving", |h| {
        let snapshot = h.runtime().snapshot();
        snapshot.state == RuntimeState::Running && snapshot.frames_submitted > before
    });
    let recovered = h.runtime().snapshot();
    assert_eq!(recovered.resources.unwrap().active.unwrap(), original);
    assert_eq!(recovered.failure.unwrap().code(), FailureCode::Backend);
}

#[test]
fn failed_reset_allocations_retry_the_same_authored_plan_with_a_new_device_lifetime() {
    let mut h = Harness::new(config(), NullConfig::default());
    let request = h.submit(plan(1, 1, 1));
    let (original, _) = h.installed(request);
    h.null.fail_next_resets(2);
    h.null.lose_device();
    h.drive("two failed resets then a rebuilt plan", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.active.is_some_and(|active| active.device != original.device))
    });
    let recovered = h.runtime().snapshot();
    let current = recovered.resources.unwrap().active.unwrap();
    assert_eq!(h.null.snapshot().reset_attempts, 3);
    assert_eq!(current.device, original.device.next().unwrap());
    assert_eq!(current.identity, original.identity);
    assert_ne!(current.generation, original.generation);
    assert_eq!(recovered.state, RuntimeState::Running);
    h.drive("rebuilt plan submits", |h| {
        h.runtime()
            .snapshot()
            .last_frame
            .is_some_and(|frame| frame.completion.device == current.device)
    });
}

#[test]
fn cancellation_wakes_a_long_failed_reset_backoff_and_retires_actual_owners() {
    let mut limits = config();
    limits.recovery_delay = Duration::from_secs(10);
    limits.max_recovery_delay = Duration::from_secs(10);
    let mut h = Harness::new(limits, NullConfig::default());
    let request = h.submit(plan(1, 1, 1));
    h.installed(request);
    h.null.fail_next_resets(10);
    h.null.lose_device();
    h.wait("first failed reset enters backoff", |h| {
        h.null.snapshot().reset_attempts == 1
            && h.runtime()
                .snapshot()
                .resources
                .is_some_and(|resources| resources.device_lost)
    });
    h.runtime().close();
    assert!(
        h.retirement.wait(DEADLINE),
        "cancellation must wake the ten-second retry wait"
    );
    assert_eq!(h.null.snapshot().reset_attempts, 1);
    assert_eq!(h.null.snapshot().live, 0);
    assert_eq!(h.runtime().snapshot().state, RuntimeState::Stopped);
}
