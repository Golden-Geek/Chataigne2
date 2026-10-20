use super::*;
use golden_core::{engine::Engine, node::Folder};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

fn wake(engine: &Engine<Folder>) -> RuntimeWakeSender {
    RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap()
}
fn rate() -> SampleRate {
    SampleRate::new(48_000).unwrap()
}
fn driver() -> Option<BackendId> {
    Some(BackendId::new("null").unwrap())
}

#[test]
fn blocked_startup_cannot_block_drop_or_release_its_worker_slot_early() {
    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, move |_, _| {
        started_tx.send(()).unwrap();
        let _ = release_rx.recv();
        Err("injected initialization failure".into())
    })
    .unwrap();
    worker.request_start_for_driver(rate(), driver()).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let consumer = thread::spawn(move || {
        drop(worker);
        dropped_tx.send(()).unwrap();
    });
    let nonblocking = dropped_rx.recv_timeout(Duration::from_secs(1));
    assert!(
        pool.try_reserve().is_err(),
        "blocked constructor retains its admitted lifetime"
    );
    assert!(SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, |_, _| Err("unused".into())).is_err());
    release_tx.send(()).unwrap();
    assert!(
        nonblocking.is_ok(),
        "drop must complete before initialization is released"
    );
    consumer.join().unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn full_start_mailbox_preserves_worker_and_coalesces_the_latest_admitted_selection() {
    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = AtomicBool::new(true);
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, move |_, _| {
        started_tx.send(()).unwrap();
        if first.swap(false, Ordering::Relaxed) {
            let _ = release_rx.recv();
        }
        Err("error".repeat(4096))
    })
    .unwrap();
    let first_request = worker.request_start_for_driver(rate(), driver()).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut latest = first_request.clone();
    for _ in 0..LIFECYCLE_COMMAND_CAPACITY {
        latest = worker.request_start_for_driver(rate(), driver()).unwrap();
    }
    assert!(matches!(
        worker.request_start_for_driver(rate(), driver()),
        Err(StartRequestError::Full)
    ));
    release_tx.send(()).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut requests = Vec::new();
    while requests.len() < 2 {
        if let SoundCardRuntimeWorkerPoll::Started(started) = worker.poll() {
            assert!(started.result.unwrap_err().len() <= MAX_START_ERROR_BYTES);
            requests.push(started.request);
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(requests, vec![first_request, latest]);
    assert!(
        started_rx.try_recv().is_err(),
        "stale selections must not construct extra native engines"
    );
    drop(worker);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}
