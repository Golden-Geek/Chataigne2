use super::*;

#[test]
fn full_prepared_storage_reuses_one_draft_and_recovers_without_rebuilding_callbacks() {
    let (engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let values = PayloadPool::new("mqtt-full-draft", 1, 32 * 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::new().unwrap();
    let mut first = build::Draft::new(raw("first", b"1"), source(&engine, id, &lifetime), false).unwrap();
    let owner = first.retain(&values, &callbacks).unwrap();
    drop(first);
    let mut second = build::Draft::new(raw("second", b"2"), source(&engine, id, &lifetime), false).unwrap();
    for _ in 0..10 {
        assert!(second.retain(&values, &callbacks).is_none());
    }
    let identity = second.prepared_identity().unwrap();
    for _ in 0..10 {
        assert!(second.retain(&values, &callbacks).is_none());
        assert_eq!(second.prepared_identity(), Some(identity));
    }
    drop(owner);
    assert!(values.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(
        second.retain(&values, &callbacks).unwrap().values[0].path_segments,
        ["second"]
    );
}

#[test]
fn queued_job_cancellation_retains_admission_and_abandoned_jobs_complete_explicitly() {
    use golden_io::RetirementPool;
    use std::num::NonZeroUsize;
    let (engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let slots = RetirementPool::new(NonZeroUsize::MIN);
    let (request, job) = Preparation::draft(
        raw("value", b"1"),
        source(&engine, id, &lifetime),
        false,
        WorkerNotifier::new(|| {}),
        &slots,
    )
    .unwrap();
    drop(request);
    assert_eq!(slots.metrics().active, 1);
    job.run();
    assert_eq!(slots.metrics().active, 0);
    let (mut request, job) = Preparation::draft(
        raw("value", b"1"),
        source(&engine, id, &lifetime),
        false,
        WorkerNotifier::new(|| {}),
        &slots,
    )
    .unwrap();
    drop(job);
    assert!(request.is_complete());
    assert!(matches!(request.poll(), Some(Err(error)) if error.contains("stopped")));
}

#[test]
fn retained_derived_capacity_survives_queued_edits_and_recovers_after_discard() {
    let (mut engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let pool = PayloadPool::new("mqtt-retained-rows", 1, 32 * 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::new().unwrap();
    let mut first = build::Draft::new(raw("value", b"42"), source(&engine, id, &lifetime), false).unwrap();
    let mut delivery = Delivery::new(first.retain(&pool, &callbacks).unwrap());
    drop(first);
    let (_, ctx) = tick(&mut engine, &mut delivery);
    drop(delivery);
    assert!(pool.try_reserve(1).is_err());
    drop(ctx);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn incoming_inbox_is_bounded_and_source_suspension_cancels_pending_work() {
    let mut inbox = Inbox::default();
    inbox.activate();
    for _ in 0..QUEUE_ITEMS + 1 {
        inbox.push(raw("value", b"42"));
    }
    assert_eq!(inbox.len(), QUEUE_ITEMS);
    assert!(inbox.feedback.take().unwrap().contains("full"));
    inbox.suspend();
    assert!(inbox.is_empty());
    assert!(!inbox.needs_service());
    assert!(inbox.feedback.take().unwrap().contains("cancelled"));
}
