use super::*;
use std::{
    collections::VecDeque,
    sync::{atomic::AtomicUsize, mpsc},
    thread,
    time::Instant,
};
#[derive(Default)]
struct Control {
    rows: Mutex<VecDeque<UltraleapRuntimePoll>>,
    polls: AtomicUsize,
    panic: AtomicBool,
    cleanup: Mutex<Option<mpsc::Receiver<()>>>,
}
struct Fake(Arc<Control>);
impl Provider for Fake {
    fn poll(&mut self) -> Result<Vec<UltraleapRuntimePoll>, String> {
        self.0.polls.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled Ultraleap panic"
        );
        let mut rows = self.0.rows.lock().unwrap();
        let count = rows.len().min(16);
        Ok(rows.drain(..count).collect())
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(rx) = self.0.cleanup.lock().unwrap().take() {
            let _ = rx.recv();
        }
    }
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn pool(count: usize) -> PayloadPool {
    PayloadPool::new("ultraleap-runtime-test", count, count * EVENT_WEIGHT).unwrap()
}
fn row(index: usize) -> UltraleapRuntimePoll {
    UltraleapRuntimePoll {
        service_connected: true,
        connected_devices: 1,
        last_event: Some(index.to_string()),
        frame: None,
    }
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < end, "Ultraleap runtime fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn blocked_open_is_nonblocking_and_retains_both_lifetime_slots_through_cancellation() {
    let owner = slots();
    let native = slots();
    let (release, rx) = mpsc::channel();
    let (entered, ready) = mpsc::channel();
    let control = Arc::new(Control::default());
    let before = Instant::now();
    let runtime = UltraleapRuntime::spawn_using(&owner, &native, pool(8), WorkerNotifier::new(|| {}), move || {
        entered.send(()).unwrap();
        rx.recv().unwrap();
        Ok(Fake(control))
    })
    .unwrap();
    assert!(before.elapsed() < Duration::from_millis(250));
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(runtime);
    assert_eq!(native.metrics().active, 1);
    assert_eq!(owner.metrics().active, 1);
    let control = Arc::new(Control::default());
    assert!(
        UltraleapRuntime::spawn_using(&owner, &native, pool(8), WorkerNotifier::new(|| {}), move || Ok(Fake(
            control
        )))
        .is_err()
    );
    release.send(()).unwrap();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn original_publication_quota_survives_draining_cloning_retirement_and_disposal() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().extend([row(0), row(1)]);
    let native = slots();
    let owner = slots();
    let sources = pool(1);
    let shared = control.clone();
    let mut runtime = UltraleapRuntime::spawn_using(
        &owner,
        &native,
        sources.clone(),
        WorkerNotifier::new(|| {}),
        move || Ok(Fake(shared)),
    )
    .unwrap();
    until(|| runtime.queued() == 1 && runtime.pressure());
    let first = runtime.poll().unwrap();
    let exported = first[0].clone();
    drop(first);
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    assert_eq!(runtime.queued(), 0);
    drop(exported);
    until(|| runtime.queued() == 1);
    let last = runtime.poll().unwrap();
    assert_eq!(last[0].prepared.poll.last_event.as_deref(), Some("1"));
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!last[0].current());
    assert!(!sources.wait_for_idle(Duration::from_millis(10)));
    drop(last);
    assert!(sources.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn full_mailbox_preserves_sixty_four_ordered_controls_and_bounds_each_actor_turn() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().extend((0..64).map(row));
    let owner = slots();
    let native = slots();
    let shared = control.clone();
    let mut runtime =
        UltraleapRuntime::spawn_using(&owner, &native, pool(128), WorkerNotifier::new(|| {}), move || {
            Ok(Fake(shared))
        })
        .unwrap();
    until(|| runtime.queued() == 32 && runtime.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(polls, control.polls.load(Ordering::Acquire));
    let mut seen = Vec::new();
    until(|| {
        let rows = runtime.poll().unwrap();
        assert!(rows.len() <= 16);
        seen.extend(
            rows.into_iter()
                .map(|row| row.prepared.poll.last_event.clone().unwrap()),
        );
        seen.len() == 64
    });
    assert_eq!(seen, (0..64).map(|i| i.to_string()).collect::<Vec<_>>());
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn native_panic_invalidates_publication_and_notifies_before_blocked_cleanup() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(row(0));
    let (release, rx) = mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(rx);
    let owner = slots();
    let native = slots();
    let shared = control.clone();
    let wakes = Arc::new(AtomicUsize::new(0));
    let observed = wakes.clone();
    let mut runtime = UltraleapRuntime::spawn_using(
        &owner,
        &native,
        pool(8),
        WorkerNotifier::new(move || {
            observed.fetch_add(1, Ordering::AcqRel);
        }),
        move || Ok(Fake(shared)),
    )
    .unwrap();
    until(|| runtime.queued() == 1);
    let rows = runtime.poll().unwrap();
    let before = wakes.load(Ordering::Acquire);
    control.panic.store(true, Ordering::Release);
    until(|| !runtime.is_running());
    assert!(!rows[0].current());
    assert!(wakes.load(Ordering::Acquire) > before);
    assert_eq!(native.metrics().active, 1);
    assert_eq!(owner.metrics().active, 1);
    release.send(()).unwrap();
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn startup_error_is_bounded_utf8_and_a_new_owner_can_recover() {
    let owner = slots();
    let native = slots();
    let mut runtime =
        UltraleapRuntime::spawn_using::<Fake>(&owner, &native, pool(8), WorkerNotifier::new(|| {}), || {
            Err("é".repeat(3000))
        })
        .unwrap();
    until(|| !runtime.is_running());
    let error = runtime.poll().err().unwrap();
    assert!(error.len() <= 1024);
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(row(1));
    let mut runtime = UltraleapRuntime::spawn_using(&owner, &native, pool(8), WorkerNotifier::new(|| {}), move || {
        Ok(Fake(control))
    })
    .unwrap();
    until(|| runtime.has_pending());
    assert!(runtime.poll().unwrap()[0].current());
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
