use super::*;

#[test]
fn panic_schedules_recovery_before_blocked_cleanup_and_a_fresh_provider_recovers() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (release, cleanup) = mpsc::channel();
    install(
        &mut engine,
        id,
        Fake {
            control: control.clone(),
            gate: None,
            cleanup: Some(cleanup),
        },
        &slots,
        pool(),
    );
    quiet(&mut engine, id);
    control.panic_poll.store(true, Ordering::Release);
    until(|| {
        turn(&mut engine);
        module(&mut engine, id).service.retry_at.is_some()
    });
    assert_eq!(slots.metrics().active, 1);
    assert!(module(&mut engine, id).transport.is_none());
    let max = path(&engine, id, "parameters/max_output");
    let start = Instant::now();
    for _ in 0..30 {
        set(&mut engine, max, ParamValue::Float(0.5));
    }
    assert!(start.elapsed() < Duration::from_millis(500));
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    attach(&mut engine, id, Control::default(), &slots, None);
    quiet(&mut engine, id);
    let connected = path(&engine, id, "connection/connected");
    assert_eq!(value(&engine, connected), ParamValue::Bool(true));
    finish(&mut engine, id, &slots);
}

#[test]
fn startup_capacity_failure_uses_absolute_escalating_deadlines_without_early_retry() {
    let (mut engine, id) = create();
    let permit = module(&mut engine, id).service.native_slot.try_reserve().unwrap();
    for delay in [250, 500, 1000] {
        if delay != 250 {
            module(&mut engine, id).service.retry_at = Some(Instant::now() - Duration::from_millis(1));
        }
        let before = Instant::now();
        let mut ctx = context(&engine);
        module(&mut engine, id).service_io(&mut ctx);
        flush(&mut engine, &mut ctx);
        let retry = module(&mut engine, id).service.retry_at.unwrap();
        assert!(retry >= before + Duration::from_millis(delay));
        assert!(retry <= before + Duration::from_millis(delay + 100));
        for _ in 0..20 {
            module(&mut engine, id).request_service();
            turn(&mut engine);
        }
        assert_eq!(module(&mut engine, id).service.retry_at, Some(retry));
        assert!(module(&mut engine, id).transport.is_none());
    }
    drop(permit);
    let slots = slots();
    attach(&mut engine, id, Control::default(), &slots, None);
    quiet(&mut engine, id);
    finish(&mut engine, id, &slots);
}

#[test]
fn eight_authored_reconfigurations_retire_old_sources_and_reuse_one_native_slot() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    let port = path(&engine, id, "connection/remote_port");
    let mut old = Vec::new();
    for round in 0..8 {
        let shared = module(&mut engine, id).transport.as_ref().unwrap().shared.clone();
        old.push(shared);
        set(&mut engine, port, ParamValue::Int(20000 + round));
        turn(&mut engine);
        module(&mut engine, id).stop_transport();
        until(|| module(&mut engine, id).service.native_slot.metrics().active == 0);
        assert!(slots.wait_for_idle(Duration::from_secs(1)));
        attach(&mut engine, id, control.clone(), &slots, None);
        quiet(&mut engine, id);
        invoke(&mut engine, id, "stopAll", vec![]);
        quiet(&mut engine, id);
        assert!(old.iter().all(|state| !state.actor.is_current()));
        assert_eq!(slots.metrics().active, 1);
    }
    assert_eq!(slots.metrics().peak, 1);
    assert_eq!(control.actions.lock().unwrap().len(), 8);
    finish(&mut engine, id, &slots);
}
