use super::super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPreparer};
use std::{
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

fn state() -> (Arc<Shared>, RevisionSource, RevisionSource, RevisionSource) {
    let actor = RevisionSource::default();
    let native = RevisionSource::default();
    let session = RevisionSource::default();
    let shared = Arc::new(Shared {
        connected: AtomicBool::new(true),
        running: AtomicBool::new(true),
        notifier: WorkerNotifier::new(|| {}),
        actor: actor.reader().guard_at(0),
        native: native.reader(),
        session: session.reader(),
        selection: Mutex::new("all".into()),
        pressure: AtomicBool::new(false),
    });
    (shared, actor, native, session)
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(2);
    while !predicate() {
        assert!(Instant::now() < end, "callback fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn blocked(pool: &PayloadPool) -> crate::app::module::script_api::NativeCallbackPayload {
    NativeCallbackPreparer::using(pool.clone())
        .try_retain(&NativeCallbackDraft::new("held", vec![]).unwrap())
        .unwrap()
}
fn spawn(
    shared: Arc<Shared>,
    slots: &RetirementPool,
    callbacks: PayloadPool,
) -> BoundedWorker<AdmittedCommand, RetainedPayload<Publication>> {
    BoundedWorker::spawn(
        "buttplug-callback-fixture",
        slots,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 4,
            event_weight: 4 * EVENT_WEIGHT,
        },
        move |ctx| {
            let events = PayloadPool::new("buttplug-callback-events", 4, 4 * EVENT_WEIGHT).unwrap();
            let mut publisher = publication::Publisher::new(&ctx, &events, &shared)
                .using_callbacks(NativeCallbackPreparer::using(callbacks));
            if publisher.publish(ButtplugWorkerEvent::DeviceAdded(ButtplugDeviceInfo {
                index: 3,
                variant_id: "device:3".into(),
                name: "fixture".into(),
                display_name: None,
                outputs: vec![],
            })) {
                let _ = ctx.receive();
            }
        },
    )
    .unwrap()
}

#[test]
fn canonical_callback_pressure_preserves_one_draft_and_admits_it_after_capacity_returns() {
    let (shared, _actor, _native, _session) = state();
    let slots = RetirementPool::new(NonZeroUsize::MIN);
    let callbacks = PayloadPool::new("buttplug-callback-quota", 1, 64 * 1024).unwrap();
    let held = blocked(&callbacks);
    let worker = spawn(shared.clone(), &slots, callbacks.clone());
    until(|| shared.pressure.load(Ordering::Acquire));
    assert_eq!(worker.event_metrics().depth, 0);
    assert_eq!(slots.metrics().active, 1);
    drop(held);
    until(|| worker.event_metrics().depth == 1);
    let mut records = vec![];
    worker.drain_events(&mut records, NonZeroUsize::MIN);
    assert_eq!(records.len(), 1);
    assert!(records[0].current());
    assert_eq!(
        records[0]
            .callback
            .as_ref()
            .unwrap()
            .payload()
            .get("callback")
            .and_then(serde_json::Value::as_str),
        Some("buttplugDeviceAdded")
    );
    drop(records);
    drop(worker);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(callbacks.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn cancellation_releases_a_native_producer_waiting_for_canonical_callback_capacity() {
    let (shared, _actor, _native, _session) = state();
    let slots = RetirementPool::new(NonZeroUsize::MIN);
    let callbacks = PayloadPool::new("buttplug-cancelled-callback", 1, 64 * 1024).unwrap();
    let held = blocked(&callbacks);
    let worker = spawn(shared.clone(), &slots, callbacks.clone());
    until(|| shared.pressure.load(Ordering::Acquire));
    let started = Instant::now();
    drop(worker);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(started.elapsed() < Duration::from_millis(100));
    drop(held);
    assert!(callbacks.wait_for_idle(Duration::from_secs(1)));
}
