use super::*;

#[test]
fn retained_notifier_does_not_keep_precise_kernel_handles_alive() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (ready, signal) = std::sync::mpsc::channel();
    let worker = BoundedWorker::<(), ()>::spawn(
        "precise-handle-owner",
        &pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |mut context| {
            context.enable_precise_deadlines().unwrap();
            let weak = Arc::downgrade(context.precise_deadline.as_ref().unwrap().signal());
            ready.send(weak).unwrap();
            assert_eq!(
                context.receive_until(Instant::now() + Duration::from_secs(3600)),
                WorkerReceive::Disconnected
            );
        },
    )
    .unwrap();
    let signal = signal.recv_timeout(Duration::from_secs(2)).unwrap();
    let notifier = worker.wake_notifier();
    assert!(signal.upgrade().is_some());
    worker.cancel();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(signal.upgrade().is_none());
    notifier.notify();
}
