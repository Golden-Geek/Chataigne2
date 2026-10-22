use super::*;

#[test]
fn observation_failures_preserve_last_values_and_the_provider_then_recover() {
    let (mut engine, id) = create();
    watch_apps(&mut engine, id, 1);
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    let cpu = path(&engine, id, "values/watched_apps_values/Watch0/cpu_ratio");
    let before = value(&engine, cpu);
    let samples = control.samples.load(Ordering::Acquire);
    control.fail_sample.store(true, Ordering::Release);
    until(|| control.samples.load(Ordering::Acquire) > samples);
    quiet(&mut engine, id);
    assert_eq!(value(&engine, cpu), before);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().running());
    assert!(module(&mut engine, id).service.retry_at.is_none());
    control.fail_sample.store(false, Ordering::Release);
    let samples = control.samples.load(Ordering::Acquire);
    until(|| control.samples.load(Ordering::Acquire) > samples);
    quiet(&mut engine, id);
    assert_ne!(value(&engine, cpu), before);
    finish(&mut engine, id, &slots);
}

#[test]
fn worker_panic_schedules_recovery_before_blocked_cleanup_releases_its_slot() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    control.panic_sample.store(true, Ordering::Release);
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut provider = fake(control);
    provider.cleanup = Some((entered, blocked));
    install(&mut engine, id, provider, &slots, payloads(32), payloads(256));
    let mut ctx = context(&engine);
    module(&mut engine, id).service(&mut ctx);
    flush(&mut engine, &mut ctx);
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    turn(&mut engine);
    assert!(module(&mut engine, id).service.runtime.is_none());
    assert!(module(&mut engine, id).service.retry_at.is_some());
    assert_eq!(slots.metrics().active, 1);
    let other = Parameter::new("Responsive", ParamValue::Int(0), ParameterChangeCheck::ValueChange);
    engine.add_node(other.into(), None);
    engine.apply_edits().unwrap();
    let other = engine.nodes.get(engine.root).unwrap().node_data().last_child.unwrap();
    set(&mut engine, other, ParamValue::Int(7));
    assert_eq!(value(&engine, other), ParamValue::Int(7));
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let recovered = Control::default();
    attach(&mut engine, id, recovered.clone(), &slots);
    quiet(&mut engine, id);
    assert_eq!(recovered.samples.load(Ordering::Acquire), 1);
    assert_eq!(
        value(&engine, path(&engine, id, "connection/connected")),
        ParamValue::Bool(true)
    );
    finish(&mut engine, id, &slots);
}

#[test]
fn startup_backoff_escalates_without_actor_polling_and_resets_only_after_a_healthy_report() {
    let (mut engine, id) = create();
    let slots = slots();
    for delay in [250, 500, 1000] {
        let wake = module(&mut engine, id).service.wake.as_ref().unwrap().notifier.clone();
        module(&mut engine, id).service.retry_at = None;
        let runtime =
            AppControlRuntime::spawn_using(wake, &slots, payloads(32), payloads(256), || -> Result<Fake, String> {
                Err("injected native setup failure".into())
            })
            .unwrap();
        until(|| !runtime.running());
        module(&mut engine, id).service.runtime = Some(runtime);
        let before = Instant::now();
        turn(&mut engine);
        let at = module(&mut engine, id).service.retry_at.unwrap();
        assert!(at.saturating_duration_since(before) >= Duration::from_millis(delay));
        assert!(slots.wait_for_idle(Duration::from_secs(1)));
        for _ in 0..10 {
            turn(&mut engine);
            assert!(module(&mut engine, id).service.runtime.is_none());
            assert_eq!(module(&mut engine, id).service.retry_at, Some(at));
        }
    }
    attach(&mut engine, id, Control::default(), &slots);
    quiet(&mut engine, id);
    finish(&mut engine, id, &slots);
    let wake = module(&mut engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let runtime =
        AppControlRuntime::spawn_using(wake, &slots, payloads(32), payloads(256), || -> Result<Fake, String> {
            Err("after healthy report".into())
        })
        .unwrap();
    until(|| !runtime.running());
    module(&mut engine, id).service.runtime = Some(runtime);
    let before = Instant::now();
    turn(&mut engine);
    let delay = module(&mut engine, id)
        .service
        .retry_at
        .unwrap()
        .saturating_duration_since(before);
    assert!(delay >= Duration::from_millis(250) && delay < Duration::from_millis(500));
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn eight_watch_reconfigurations_keep_one_native_lifetime_and_recover_payload_pressure() {
    let (mut engine, id) = create();
    let watch = watch_apps(&mut engine, id, 1)[0];
    let slots = slots();
    let reports = payloads(1);
    let control = Control::default();
    install(
        &mut engine,
        id,
        fake(control.clone()),
        &slots,
        reports.clone(),
        payloads(256),
    );
    quiet(&mut engine, id);
    for n in 1..=8 {
        let path_value = format!("fixture/round{n}.exe");
        set(&mut engine, watch, ParamValue::File(path_value.clone()));
        quiet(&mut engine, id);
        assert_eq!(slots.metrics().active, 1);
        let configs = control.configs.lock().unwrap();
        assert_eq!(configs.last().unwrap().apps[0].target_path, path_value);
    }
    let held = reports.try_retain((), 2 * 1024 * 1024).unwrap();
    let sampled = control.samples.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(550));
    quiet(&mut engine, id);
    assert_eq!(control.samples.load(Ordering::Acquire), sampled);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().running());
    drop(held);
    until(|| control.samples.load(Ordering::Acquire) > sampled);
    quiet(&mut engine, id);
    finish(&mut engine, id, &slots);
    assert!(reports.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn action_failure_reports_failure_without_restarting_or_replaying_the_command() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    control.fail_action.store(true, Ordering::Release);
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    invoke(&mut engine, id, "failed.exe");
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = context(&engine);
    module(&mut engine, id).service(&mut ctx);
    flush(&mut engine, &mut ctx);
    until(|| {
        module(&mut engine, id)
            .service
            .pending
            .front()
            .is_some_and(Receipt::complete)
    });
    let mut ctx = context(&engine);
    module(&mut engine, id).service(&mut ctx);
    flush(&mut engine, &mut ctx);
    assert_eq!(callbacks(&engine.inbox.events, "appControlCommandFailed"), 1);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().running());
    control.fail_action.store(false, Ordering::Release);
    invoke(&mut engine, id, "recovered.exe");
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 2);
    finish(&mut engine, id, &slots);
}
