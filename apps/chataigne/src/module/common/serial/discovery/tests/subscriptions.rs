use super::*;
use std::sync::Mutex;

fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn subscriber_capacity_and_identity_exhaustion_precede_native_startup() {
    let service = Service::new(|| panic!("no native startup"));
    {
        let mut state = service.state.lock().unwrap();
        for id in 0..MAX_SUBSCRIBERS as u64 {
            state.subscribers.insert(id, WorkerNotifier::new(|| {}));
        }
    }
    assert!(service.register(WorkerNotifier::new(|| {})).is_err());
    assert_eq!(service.pool.metrics().active, 0);
    let mut state = service.state.lock().unwrap();
    state.subscribers.clear();
    state.next_id = u64::MAX;
    drop(state);
    assert!(service.register(WorkerNotifier::new(|| {})).is_err());
    assert_eq!(service.pool.metrics().active, 0);
}
#[test]
fn last_unsubscribe_fences_blocked_producer_and_coarsely_retries_the_single_slot() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = Mutex::new(release_rx);
    let calls = AtomicUsize::new(0);
    let service = Service::new(move || {
        if calls.fetch_add(1, Ordering::AcqRel) == 0 {
            started_tx.send(()).unwrap();
            release.lock().unwrap().recv().unwrap();
        }
        Ok(vec![])
    });
    let registration = service.register(WorkerNotifier::new(|| {})).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let old_epoch = service.state.lock().unwrap().epoch;
    let started = Instant::now();
    drop(registration);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(service.pool.metrics().active, 1);
    let registration = service.register(WorkerNotifier::new(|| {})).unwrap();
    assert!(registration.retry_after().is_some());
    service.publish(old_epoch, vec![], Some("stale catalog".into()));
    assert_ne!(registration.snapshot().error.as_deref(), Some("stale catalog"));
    release_tx.send(()).unwrap();
    assert!(service.pool.wait_for_idle(Duration::from_secs(2)));
    service.state.lock().unwrap().retry_at = Some(Instant::now());
    wait(|| registration.snapshot().catalog.is_some());
    assert!(registration.snapshot().error.is_none());
    drop(registration);
    assert!(service.pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn unexpected_producer_exit_notifies_owner_and_recovers_without_a_periodic_module_poll() {
    let calls = AtomicUsize::new(0);
    let service = Service::new(move || {
        assert_ne!(
            calls.fetch_add(1, Ordering::AcqRel),
            0,
            "injected discovery producer failure"
        );
        Ok(vec![])
    });
    let notices = Arc::new(AtomicUsize::new(0));
    let output = notices.clone();
    let registration = service
        .register(WorkerNotifier::new(move || {
            output.fetch_add(1, Ordering::AcqRel);
        }))
        .unwrap();
    wait(|| notices.load(Ordering::Acquire) > 0);
    assert!(service.pool.wait_for_idle(Duration::from_secs(2)));
    wait(|| registration.snapshot().catalog.is_some());
    assert!(registration.snapshot().error.is_none());
    drop(registration);
    assert!(service.pool.wait_for_idle(Duration::from_secs(2)));
}
