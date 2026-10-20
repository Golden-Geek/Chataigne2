use super::*;
use std::sync::mpsc;

#[test]
fn configuration_bounds_items_labels_and_retained_capacity() {
    let mut next = config();
    next.signals = vec![next.signals[0].clone(); generators::MAX_ITEMS + 1];
    assert!(next.weight().is_err());
    let mut next = config();
    next.signals[0].label = "x".repeat(generators::MAX_LABEL_BYTES + 1);
    assert!(next.weight().is_err());
    let mut next = config();
    next.signals[0].label = String::with_capacity(generators::MAX_CONFIG_WEIGHT);
    assert!(
        next.weight().is_err(),
        "retained capacity matters even for empty strings"
    );
}

#[test]
fn item_and_byte_pressure_reject_without_restarting_or_losing_accepted_commands() {
    for byte_pressure in [false, true] {
        let pool = golden_io::RetirementPool::new(NonZeroUsize::MIN);
        let (resume, gate) = mpsc::channel();
        let (result, received) = mpsc::channel();
        let mut first = config();
        first.signals[0].label = "first".into();
        let mut second = config();
        second.signals[0].label = "second".into();
        let mut limits = generators::limits();
        limits.command_items = 1;
        if byte_pressure {
            limits.command_items = 2;
            limits.command_weight = first.weight().unwrap() + second.weight().unwrap() - 1;
        }
        let worker = BoundedWorker::spawn("generator-admission", &pool, limits, move |context| {
            gate.recv().unwrap();
            let mut labels = Vec::new();
            while let Some(command) = context.receive() {
                if let SignalWorkerCommand::Configure(config) = command {
                    labels.push(config.signals[0].label.clone());
                }
            }
            result.send(labels).unwrap();
        })
        .unwrap();
        let mut runtime = SignalRuntimeHandle { worker };
        runtime.configure(first).unwrap();
        assert_eq!(runtime.configure(second), Err(AdmissionError::Full));
        assert!(!runtime.is_closed());
        runtime.worker.finish_admission();
        resume.send(()).unwrap();
        assert_eq!(received.recv_timeout(Duration::from_secs(3)).unwrap(), ["first"]);
        assert!(pool.wait_for_idle(Duration::from_secs(3)));
    }
}

#[test]
fn publication_backpressure_preserves_all_rows_and_counts_in_bounded_actor_turns() {
    let pool = golden_io::RetirementPool::new(NonZeroUsize::MIN);
    let count = EVENT_ROWS * 3 + 7;
    let mut event = SignalWorkerEvent::default();
    for index in 0..count {
        let item_id = NodeId(index as u64 + 1);
        event.samples.insert(
            item_id,
            SignalWorkerSample {
                item_id,
                label: "row".into(),
                value: 0.5,
                cycle: 7,
                cycles: 7,
            },
        );
    }
    let mut limits = generators::limits();
    limits.event_items = 1;
    let (done, published) = mpsc::channel();
    let worker = BoundedWorker::spawn("generator-backpressure", &pool, limits, move |context| {
        done.send(publish_event(&context, event)).unwrap();
    })
    .unwrap();
    let runtime = SignalRuntimeHandle { worker };
    let deadline = Instant::now() + Duration::from_secs(3);
    while runtime.worker.event_metrics().depth == 0 {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(matches!(published.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let mut ids = HashSet::new();
    let mut counts = 0;
    while ids.len() < count {
        assert!(Instant::now() < deadline);
        if let Some(event) = runtime.take_event() {
            assert!(event.samples.len() <= EVENT_ROWS);
            for row in event.samples.into_values() {
                assert!(ids.insert(row.item_id));
                counts += row.cycles;
            }
        } else {
            std::thread::yield_now();
        }
    }
    assert_eq!(counts, (count * 7) as i64);
    assert!(published.recv_timeout(Duration::from_secs(3)).unwrap());
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn drop_does_not_join_blocked_cleanup_and_lifetime_capacity_recovers_after_exit() {
    let pool = golden_io::RetirementPool::new(NonZeroUsize::MIN);
    let (entered, ready) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let worker = BoundedWorker::spawn("generator-cleanup", &pool, generators::limits(), move |_context| {
        entered.send(()).unwrap();
        gate.recv().unwrap();
    })
    .unwrap();
    let mut runtime = SignalRuntimeHandle { worker };
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    let (done, dropped) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        runtime.stop();
        drop(runtime);
        done.send(()).unwrap();
    });
    let nonblocking = dropped.recv_timeout(Duration::from_secs(3)).is_ok();
    let replacement = BoundedWorker::<(), ()>::spawn("generator-full", &pool, generators::limits(), |_| {});
    release.send(()).unwrap();
    owner.join().unwrap();
    assert!(nonblocking, "generator destruction must not wait for native cleanup");
    assert!(matches!(replacement, Err(golden_io::WorkerStartError::Capacity(_))));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    let replacement =
        BoundedWorker::<(), ()>::spawn("generator-recovered", &pool, generators::limits(), |_| {}).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    drop(replacement);
}

#[test]
fn stale_reset_ids_cannot_grow_runtime_state() {
    let origin = Instant::now();
    let mut worker = WorkerState::new(config(), origin);
    let slot = SignalEventSlot::new();
    worker.service(origin, Some(SignalWorkerCommand::Reset(vec![NodeId(999)])), &slot);
    assert!(!worker.states.contains_key(&NodeId(999)));
    assert!(worker.states.len() <= 1);
}

#[test]
fn oversized_curve_keys_and_script_capacity_are_rejected() {
    use golden_core::node::{Curve, CurveEasing, CurveKey};
    let mut next = config();
    next.signals[0].curve = Some(Curve::new(
        (0..=generators::MAX_CURVE_KEYS)
            .map(|i| CurveKey::new(i as f64, 0.0, CurveEasing::Linear))
            .collect(),
    ));
    assert!(next.weight().is_err());
    next.signals[0].curve = Some(Curve::new(vec![CurveKey::new(
        0.0,
        0.0,
        CurveEasing::Script {
            source: String::with_capacity(generators::MAX_CONFIG_WEIGHT / 2),
        },
    )]));
    assert!(next.weight().is_err());
}
