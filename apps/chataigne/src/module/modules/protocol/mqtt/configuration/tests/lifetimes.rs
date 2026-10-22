use super::*;

#[test]
fn cancellation_retains_admission_until_the_queued_job_releases_its_snapshot() {
    let (engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (request, job) = draft(&snapshot, id, &slots);
    assert!(!request.is_complete());
    drop(request);
    assert_eq!(slots.metrics().active, 1);
    assert!(Preparation::draft(&snapshot, id, None, WorkerNotifier::new(|| {}), &slots).is_err());
    job.run();
    assert_eq!(slots.metrics().active, 0);
    let (mut request, job) = draft(&snapshot, id, &slots);
    job.run();
    assert!(request.poll().unwrap().is_some());
}

#[test]
fn abandoned_job_returns_a_retryable_completion_instead_of_waiting_forever() {
    let (engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (mut request, job) = draft(&snapshot, id, &slots);
    drop(job);
    assert!(request.is_complete());
    let error = request.poll().unwrap_err();
    assert!(error.retryable);
    assert!(error.message.contains("stopped"));
    assert_eq!(slots.metrics().active, 1);
    drop(request);
    assert_eq!(slots.metrics().active, 0);
}

#[test]
fn ready_results_keep_one_admitted_configuration_across_retries_and_unchanged_capture() {
    let (engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (mut request, job) = draft(&snapshot, id, &slots);
    job.run();
    let (first, unchanged) = request.poll().unwrap().unwrap();
    assert!(!unchanged);
    let (retry, _) = request.poll().unwrap().unwrap();
    assert!(std::ptr::eq(&*first, &*retry));
    assert_eq!(slots.metrics().active, 1);
    drop(request);
    let (mut request, job) =
        Preparation::draft(&snapshot, id, Some(first.clone()), WorkerNotifier::new(|| {}), &slots).unwrap();
    job.run();
    let (same, unchanged) = request.poll().unwrap().unwrap();
    assert!(unchanged);
    assert!(std::ptr::eq(&*first, &*same));
}

#[test]
fn waiting_preparation_is_snapshot_free_and_disable_cancels_without_waiting_for_the_job() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (request, job) = draft(&snapshot, id, &slots);
    let time = engine.time;
    let node = module(&mut engine, id);
    node.configuration = Some(request);
    node.transport_dirty = true;
    assert!(!node.worker_wake_requires_tree_snapshot());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    node.on_worker_wake(&mut ctx);
    assert!(node.configuration.is_some());
    assert!(ctx.tree_snapshot().is_none());
    node.on_effective_enabled_changed(&mut ctx, false);
    assert!(node.configuration.is_none());
    assert!(!node.transport_dirty);
    assert_eq!(slots.metrics().active, 1);
    job.run();
    assert_eq!(slots.metrics().active, 0);
}

#[test]
fn invalid_preparation_completes_passively_and_stops_without_native_start_or_retry() {
    let (mut engine, id) = create_mqtt_module();
    let host = find_path(&engine, id, "connection/remote_host").unwrap();
    set(&mut engine, host, ParamValue::Str("x".repeat(1025)));
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (request, job) = draft(&snapshot, id, &slots);
    let time = engine.time;
    let node = module(&mut engine, id);
    node.configuration_key = Key::capture(&snapshot, id);
    node.configuration = Some(request);
    node.transport_dirty = true;
    job.run();
    assert!(node.worker_wake_requires_tree_snapshot());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    ctx.set_tree_snapshot(snapshot);
    node.on_worker_wake(&mut ctx);
    assert!(node.configuration.is_none());
    assert!(node.transport.is_none());
    assert!(node.last_transport_config.is_none());
    assert!(!node.transport_dirty);
    assert!(node.retry_at.is_none());
    assert!(!node.worker_wake_requires_tree_snapshot());
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn failed_preparation_uses_a_coarse_retry_and_an_authored_change_cancels_the_deadline() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (request, job) = draft(&snapshot, id, &slots);
    drop(job);
    let time = engine.time;
    let node = module(&mut engine, id);
    node.configuration = Some(request);
    node.transport_dirty = true;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    ctx.set_tree_snapshot(snapshot);
    node.on_worker_wake(&mut ctx);
    assert!(node.configuration.is_none());
    assert!(node.transport.is_none());
    assert!(!node.transport_dirty);
    assert!(node.retry_at.is_some());
    assert!(!node.worker_wake_requires_tree_snapshot());
    let host = find_path(&engine, id, "connection/remote_host").unwrap();
    set(&mut engine, host, ParamValue::Str("localhost".into()));
    let snapshot = engine.process_tree_snapshot();
    let node = module(&mut engine, id);
    node.notice_configuration(&snapshot);
    assert!(node.retry_at.is_none());
    assert!(node.transport_dirty);
}
