use super::*;

#[test]
fn native_output_cannot_open_or_inject_in_test_builds() {
    assert!(native::MouseOutputController::create().is_err());
    let mut native = native::MouseOutputController;
    for action in [
        MouseButtonAction::Click,
        MouseButtonAction::Press,
        MouseButtonAction::Release,
    ] {
        assert!(native
            .execute(Request::Button {
                button: MouseButtonKind::Left,
                action
            })
            .is_err());
    }
    for request in [
        Request::Move {
            x: 1.0,
            y: 2.0,
            coordinate: MouseMoveCoordinate::Relative,
            units: MouseMoveUnits::Pixels,
        },
        Request::Move {
            x: 0.5,
            y: 0.5,
            coordinate: MouseMoveCoordinate::Absolute,
            units: MouseMoveUnits::Normalized,
        },
        Request::Scroll {
            vertical: 1,
            horizontal: 1,
        },
    ] {
        assert!(native.execute(request).is_err());
    }
}
#[test]
fn sixty_four_original_sources_cover_native_receipts_and_final_disposal() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (runtime, owner, global, pool) = spawn(control.clone());
    let admission = slots(64);
    let ingress = slots(64);
    let mut receipts = vec![submit(
        &runtime,
        source(&runtime, &admission, &ingress, MouseButtonKind::Left),
    )];
    until(|| control.calls.lock().unwrap().len() == 1);
    for _ in 1..64 {
        receipts.push(submit(
            &runtime,
            source(&runtime, &admission, &ingress, MouseButtonKind::Middle),
        ));
    }
    assert_eq!(admission.metrics().active, 64);
    assert!(admission.try_reserve().is_err());
    release.send(()).unwrap();
    until(|| receipts.iter().all(Receipt::complete));
    assert_eq!(control.calls.lock().unwrap().len(), 64);
    assert!(admission.try_reserve().is_err());
    drop(runtime);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(receipts.iter().all(|receipt| receipt.take().is_some()));
    assert!(admission.try_reserve().is_err());
    drop(receipts);
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(admission.metrics().active, 0);
}
#[test]
fn unarrived_ingress_refuses_native_submission_until_the_same_source_arrives() {
    let control = Control::default();
    let (runtime, _, global, _) = spawn(control.clone());
    let admission = slots(1);
    let ingress = slots(1);
    let source = source(&runtime, &admission, &ingress, MouseButtonKind::Left);
    let source = runtime.submit(source).err().unwrap();
    assert!(control.calls.lock().unwrap().is_empty());
    assert_eq!(ingress.metrics().active, 1);
    let receipt = submit(&runtime, source);
    until(|| receipt.complete());
    assert_eq!(control.calls.lock().unwrap().len(), 1);
    drop(runtime);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn blocked_open_is_nonblocking_and_keeps_lifetime_capacity_until_it_finishes() {
    let owner = slots(1);
    let global = slots(1);
    let input = pool();
    let (release, gate) = mpsc::channel();
    let control = Control::default();
    let start = Instant::now();
    let runtime = Runtime::spawn_using(&owner, &global, input, WorkerNotifier::new(|| {}), move || {
        gate.recv().unwrap();
        Ok(Fake(control))
    })
    .unwrap();
    assert!(start.elapsed() < Duration::from_millis(100));
    drop(runtime);
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(global.metrics().active, 1);
    release.send(()).unwrap();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn panic_marks_started_unconfirmed_and_suffix_not_sent_before_blocked_cleanup() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (retire, cleanup) = mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(cleanup);
    control.panic.store(true, Ordering::Release);
    let (runtime, owner, global, _) = spawn(control.clone());
    let admission = slots(4);
    let ingress = slots(4);
    let first = submit(&runtime, source(&runtime, &admission, &ingress, MouseButtonKind::Left));
    until(|| control.calls.lock().unwrap().len() == 1);
    let suffix = submit(
        &runtime,
        source(&runtime, &admission, &ingress, MouseButtonKind::Middle),
    );
    release.send(()).unwrap();
    until(|| first.complete() && suffix.complete() && !runtime.is_running());
    assert_eq!(first.take(), Some(Outcome::Unconfirmed));
    assert_eq!(suffix.take(), Some(Outcome::NotSent));
    assert_eq!(control.calls.lock().unwrap().len(), 1);
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(global.metrics().active, 1);
    drop(runtime);
    retire.send(()).unwrap();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn retirement_preserves_completed_result_and_cancels_unstarted_work_without_replay() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (runtime, owner, global, _) = spawn(control.clone());
    let admission = slots(2);
    let ingress = slots(2);
    let first = submit(&runtime, source(&runtime, &admission, &ingress, MouseButtonKind::Left));
    until(|| control.calls.lock().unwrap().len() == 1);
    let next = submit(
        &runtime,
        source(&runtime, &admission, &ingress, MouseButtonKind::Middle),
    );
    let start = Instant::now();
    drop(runtime);
    assert!(start.elapsed() < Duration::from_millis(100));
    release.send(()).unwrap();
    until(|| first.complete() && next.complete());
    assert!(matches!(first.take(), Some(Outcome::Completed(Ok(_)))));
    assert_eq!(next.take(), Some(Outcome::NotSent));
    assert_eq!(control.calls.lock().unwrap().len(), 1);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn native_error_diagnostic_is_bounded_and_a_later_explicit_call_still_works() {
    let control = Control::default();
    control.fail.store(true, Ordering::Release);
    let (runtime, _, global, _) = spawn(control.clone());
    let admission = slots(2);
    let ingress = slots(2);
    let first = submit(&runtime, source(&runtime, &admission, &ingress, MouseButtonKind::Left));
    until(|| first.complete());
    let Some(Outcome::Completed(Err(error))) = first.take() else {
        panic!("native failure");
    };
    assert!(error.len() <= 1024);
    assert!(error.is_char_boundary(error.len()));
    let second = submit(
        &runtime,
        source(&runtime, &admission, &ingress, MouseButtonKind::Middle),
    );
    until(|| second.complete());
    assert!(matches!(second.take(), Some(Outcome::Completed(Ok(_)))));
    assert_eq!(control.calls.lock().unwrap().len(), 2);
    drop(runtime);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
}
