use super::*;
mod catalog;
mod subscriptions;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::Instant,
};

impl SerialDiscoveryRegistration {
    pub(crate) fn isolated_empty_for_test(notifier: WorkerNotifier) -> Self {
        Service::new(|| Ok(Vec::new()))
            .register(notifier)
            .expect("isolated serial discovery registration should succeed")
    }
}

#[test]
fn blocked_enumeration_keeps_the_single_native_slot_without_joining_or_publishing_after_stop() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let published = Arc::new(AtomicUsize::new(0));
    let output = published.clone();
    let mut worker = SerialDiscoveryWorker::spawn_with(
        &pool,
        move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(Vec::new())
        },
        move |_, _| {
            output.fetch_add(1, Ordering::AcqRel);
        },
    )
    .unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let started = Instant::now();
    worker.stop();
    drop(worker);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    assert!(
        SerialDiscoveryWorker::spawn_with(&pool, || panic!("no replacement"), |_, _| panic!("no publication")).is_err()
    );
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(published.load(Ordering::Acquire), 0);
    let output = published.clone();
    let mut recovered = SerialDiscoveryWorker::spawn_with(
        &pool,
        || Ok(Vec::new()),
        move |_, _| {
            output.fetch_add(1, Ordering::AcqRel);
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while published.load(Ordering::Acquire) == 0 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    recovered.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn enumeration_errors_and_recovery_keep_the_last_good_catalog() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (output, input) = mpsc::channel();
    let mut attempt = 0;
    let mut worker = SerialDiscoveryWorker::spawn_with(
        &pool,
        move || {
            attempt += 1;
            if attempt == 2 {
                return Err("injected enumeration failure".into());
            }
            Ok(vec![DiscoveredSerialPort {
                port_name: "fixture".into(),
                label: "fixture".into(),
                tags: Vec::new(),
                details: String::new(),
            }])
        },
        move |ports, error| {
            output.send((ports, error)).unwrap();
        },
    )
    .unwrap();
    let first = input.recv_timeout(Duration::from_secs(2)).unwrap();
    let failed = input.recv_timeout(Duration::from_secs(2)).unwrap();
    let recovered = input.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(first.1.is_none());
    assert_eq!(failed.0, first.0);
    assert_eq!(failed.1.as_deref(), Some("injected enumeration failure"));
    assert_eq!(recovered, first);
    worker.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
