use super::*;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn shutdown_notification_waits_for_publication_ownership_to_quiesce() {
    let shared = Arc::new(Shared::default());
    let publication = shared.latest.lock().unwrap();
    let stopped_shared = shared.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let stopper = std::thread::spawn(move || {
        // The renderer cache is thread-local; move only the shared mailbox into this
        // test thread and construct a source without renderer imports here.
        let mut source = Source {
            shared: stopped_shared,
            join: None,
            current: None,
            imports: None,
            import_report: None,
        };
        started_tx.send(()).unwrap();
        source.stop();
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (notified_tx, notified_rx) = mpsc::channel();
    let observed_shared = shared.clone();
    let observer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !observed_shared.stop.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline, "stop notification never arrived");
            std::thread::yield_now();
        }
        notified_tx.send(()).unwrap();
    });
    // The real stop path is blocked on a held publication. A worker must not be notified
    // until that ownership can be cleared, regardless of which thread runs first.
    assert_eq!(
        notified_rx.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    drop(publication);
    notified_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    stopper.join().unwrap();
    observer.join().unwrap();
}
