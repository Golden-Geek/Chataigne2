use crate::{RetirementPool, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[test]
fn released_reservation_notifies_after_capacity_is_available_and_outside_the_pool_lock() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let permit = pool.try_reserve().unwrap();
    let revision = pool.availability();
    let checkpoint = revision.current();
    let (tx, rx) = mpsc::channel();
    let observer = pool.clone();
    let _subscription = revision.guard_at(checkpoint).subscribe(WorkerNotifier::new(move || {
        assert_eq!(observer.metrics().active, 0);
        tx.send(()).unwrap();
    }));
    assert!(pool.try_reserve().is_err());
    assert!(rx.try_recv().is_err());
    drop(permit);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(revision.current() > checkpoint);
    assert!(pool.try_reserve().is_ok());
    assert_eq!(pool.metrics().peak, 1);
}

#[test]
fn cleanup_release_and_late_subscription_cannot_lose_the_admission_wake() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let revision = pool.availability();
    let checkpoint = revision.current();
    let (release, released) = mpsc::channel();
    let (started, running) = mpsc::channel();
    let cleaned = Arc::new(AtomicBool::new(false));
    let cleanup = cleaned.clone();
    pool.try_retire("release-after-cleanup", (), move |_| {
        started.send(()).unwrap();
        released.recv().unwrap();
        cleanup.store(true, Ordering::Release);
    })
    .unwrap();
    running.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.try_reserve().is_err());
    let (notify, notified) = mpsc::channel();
    let observer = cleaned.clone();
    let _subscription = revision.guard_at(checkpoint).subscribe(WorkerNotifier::new(move || {
        assert!(observer.load(Ordering::Acquire));
        notify.send(()).unwrap();
    }));
    assert!(notified.try_recv().is_err());
    release.send(()).unwrap();
    notified.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let (late, notification) = mpsc::channel();
    let _late = revision.guard_at(checkpoint).subscribe(WorkerNotifier::new(move || {
        late.send(()).unwrap();
    }));
    notification.recv_timeout(Duration::from_secs(2)).unwrap();
}
