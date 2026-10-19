use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

fn counter() -> (WorkerNotifier, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let signal = count.clone();
    (
        WorkerNotifier::new(move || {
            signal.fetch_add(1, Ordering::Relaxed);
        }),
        count,
    )
}

#[test]
fn repeated_deadlines_coalesce_and_fire_once() {
    let timer = WakeTimer::new();
    let (notifier, count) = counter();
    let wake = timer.register(notifier).unwrap();
    wake.wake_after(Duration::from_secs(2));
    for _ in 0..4096 {
        wake.wake_after(Duration::from_secs(3));
    }
    let mut state = timer.state.lock().unwrap();
    assert_eq!(state.registrations.len(), 1);
    let (due, next) = WakeTimer::collect_due(&mut state, Instant::now());
    assert!(due.is_empty());
    assert!(next.is_some());
    let (due, next) = WakeTimer::collect_due(&mut state, Instant::now() + Duration::from_secs(4));
    assert_eq!(due.len(), 1);
    assert!(next.is_none());
    assert!(
        WakeTimer::collect_due(&mut state, Instant::now() + Duration::from_secs(5))
            .0
            .is_empty()
    );
    drop(state);
    for notifier in due {
        notifier.notify();
    }
    assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn dropping_or_cancelling_a_source_removes_its_deadline_and_recovers_capacity() {
    let timer = WakeTimer::new();
    let mut sources = (0..DELAYED_WAKE_CAPACITY)
        .map(|_| timer.register(counter().0).unwrap())
        .collect::<Vec<_>>();
    assert!(timer.register(counter().0).is_err());
    sources[0].wake_after(Duration::from_secs(10));
    drop(sources.remove(0));
    let replacement = timer.register(counter().0).unwrap();
    replacement.wake_after(Duration::from_secs(1));
    replacement.cancel_deadline();
    let mut state = timer.state.lock().unwrap();
    let (due, next) = WakeTimer::collect_due(&mut state, Instant::now() + Duration::from_secs(20));
    assert!(due.is_empty());
    assert!(next.is_none());
}

#[test]
fn a_live_timer_receives_a_deadline_after_idle_and_can_be_reused() {
    let (sender, receiver) = mpsc::channel();
    let wake = DelayedNotifier::new(WorkerNotifier::new(move || {
        let _ = sender.send(());
    }))
    .unwrap();
    for _ in 0..2 {
        wake.wake_after(Duration::from_millis(1));
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}
