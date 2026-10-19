use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

#[test]
fn shutdown_cancels_queued_work_without_applying_it() {
    let actor = ControlActor::spawn_with_config(
        "cancel-probe",
        (),
        ControlActorConfig {
            pending_capacity: std::num::NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    let handle = actor.handle();
    let metrics = handle.metrics();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let active = handle
        .submit(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        })
        .unwrap();
    entered_rx.recv().unwrap();
    let applied = Arc::new(AtomicUsize::new(0));
    let queued_applied = applied.clone();
    let queued = handle
        .submit(move |_| {
            queued_applied.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    let stopping = thread::spawn(move || drop(actor));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let disconnected = loop {
        let error = handle.submit(|_| ()).err().expect("full or stopped actor");
        if error.kind() == ControlErrorKind::Disconnected {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
        thread::yield_now();
    };
    release_tx.send(()).unwrap();
    active.wait().unwrap();
    assert_eq!(queued.wait().unwrap_err().kind(), ControlErrorKind::Cancelled);
    stopping.join().unwrap();
    assert!(disconnected);
    assert_eq!(applied.load(Ordering::SeqCst), 0);
    assert_eq!(metrics.snapshot().control_queue_depth, 0);
}
