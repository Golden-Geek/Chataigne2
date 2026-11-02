use super::*;

#[test]
fn guards_fence_advances_other_lifetimes_and_owner_retirement() {
    let source = RevisionSource::default();
    let reader = source.reader();
    let zero = reader.guard_at(0);
    assert!(zero.is_current());
    assert_eq!(source.advance(), 1);
    assert!(!zero.is_current());
    assert!(!reader.guard_at(0).is_current());
    let current = reader.guard_at(1);
    let other = RevisionSource::default();
    other.advance();
    drop(other);
    assert!(current.is_current());
    drop(source);
    assert!(!current.is_current());
    assert!(!reader.guard_at(reader.current()).is_current());
}

#[test]
fn cross_thread_advance_invalidates_existing_guard() {
    let source = RevisionSource::default();
    let guard = source.reader().guard_at(0);
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        source.advance();
        send.send(source).unwrap();
    });
    let source = receive.recv().unwrap();
    assert!(!guard.is_current());
    assert!(source.reader().guard_at(1).is_current());
    worker.join().unwrap();
}

#[test]
fn exhausted_revision_cannot_wrap_and_revive_an_old_guard() {
    let source = RevisionSource::default();
    let old = source.reader().guard_at(0);
    source.state.revision.store(u64::MAX, Ordering::Release);
    assert!(std::panic::catch_unwind(|| source.advance()).is_err());
    assert_eq!(source.reader().current(), u64::MAX);
    assert!(!old.is_current());
}

fn notifications() -> (WorkerNotifier, std::sync::mpsc::Receiver<()>) {
    let (send, receive) = std::sync::mpsc::channel();
    (
        WorkerNotifier::new(move || {
            send.send(()).unwrap();
        }),
        receive,
    )
}

#[test]
fn subscriptions_notify_once_on_advance_or_retirement_and_cancel_on_drop() {
    let source = RevisionSource::default();
    let (notifier, receive) = notifications();
    let guard = source.reader().guard_at(0);
    let subscription = guard.subscribe(notifier.clone());
    assert!(receive.try_recv().is_err());
    source.advance();
    assert_eq!(receive.try_recv(), Ok(()));
    source.advance();
    assert!(receive.try_recv().is_err(), "one-shot watch");
    drop(subscription);

    let current = source.reader().guard_at(2);
    let cancelled = current.subscribe(notifier.clone());
    drop(cancelled);
    source.advance();
    assert!(receive.try_recv().is_err(), "cancelled watch");

    let _retirement = source.reader().guard_at(3).subscribe(notifier.clone());
    drop(source);
    assert_eq!(receive.try_recv(), Ok(()));
    let _stale = current.subscribe(notifier);
    assert_eq!(receive.try_recv(), Ok(()), "late subscription cannot miss invalidation");
}

#[test]
fn concurrent_registration_cannot_miss_invalidation() {
    for _ in 0..64 {
        let source = RevisionSource::default();
        let guard = source.reader().guard_at(0);
        let (notifier, receive) = notifications();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let worker_barrier = barrier.clone();
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            source.advance();
            source
        });
        barrier.wait();
        let _subscription = guard.subscribe(notifier);
        let source = worker.join().unwrap();
        assert_eq!(receive.try_recv(), Ok(()));
        assert!(receive.try_recv().is_err(), "exactly one invalidation");
        drop(source);
    }
}

#[test]
fn callbacks_can_subscribe_without_holding_the_revision_registry_lock() {
    let source = RevisionSource::default();
    let reader = source.reader();
    let subscriptions = Arc::new(Mutex::new(Vec::new()));
    let retained = subscriptions.clone();
    let (notifier, receive) = notifications();
    let _first = reader.guard_at(0).subscribe(WorkerNotifier::new(move || {
        retained
            .lock()
            .unwrap()
            .push(reader.guard_at(reader.current()).subscribe(notifier.clone()));
    }));
    source.advance();
    assert!(receive.try_recv().is_err());
    source.advance();
    assert_eq!(receive.try_recv(), Ok(()));
}

#[test]
fn guards_retain_only_fixed_storage_and_source_retirement_frees_the_watch_registry() {
    let source = RevisionSource::default();
    let guard = source.reader().guard_at(0);
    let expected = size_of::<State>() + size_of::<Registry>() + 4 * size_of::<usize>();
    assert_eq!(guard.retained_heap_bytes(), expected);
    let (notifier, receive) = notifications();
    let subscriptions = (0..128).map(|_| guard.subscribe(notifier.clone())).collect::<Vec<_>>();
    assert_eq!(
        guard.retained_heap_bytes(),
        expected,
        "source-owned storage is not retained by guards"
    );
    assert_eq!(
        guard.reader.registry.strong_count(),
        1,
        "only the source owns the registry"
    );
    assert!(source.registry.watchers.lock().unwrap().capacity() >= 128);
    drop(source);
    assert!(
        guard.reader.registry.upgrade().is_none(),
        "Vec backing freed despite retained guards/subscriptions"
    );
    assert!(!guard.is_current());
    assert_eq!(receive.try_iter().count(), subscriptions.len());
    assert_eq!(guard.retained_heap_bytes(), expected);
    let _late = guard.subscribe(notifier);
    assert_eq!(receive.try_recv(), Ok(()));
}
