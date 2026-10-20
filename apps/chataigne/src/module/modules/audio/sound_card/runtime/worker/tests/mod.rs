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

#[test]
fn disabling_during_startup_discards_the_result_and_queued_constructors_then_resumes() {
    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = AtomicBool::new(true);
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, move |rate, _| {
        started_tx.send(rate).unwrap();
        if first.swap(false, Ordering::Relaxed) {
            let _ = release_rx.recv();
        }
        Err("injected startup outcome".into())
    })
    .unwrap();
    worker.request_start_for_driver(rate(), driver()).unwrap();
    assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), rate());
    worker
        .request_start_for_driver(SampleRate::new(44_100).unwrap(), driver())
        .unwrap();
    worker.set_enabled(false);
    release_tx.send(()).unwrap();
    assert!(
        started_rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "disabled queued selection must not construct"
    );
    assert!(
        matches!(worker.poll(), SoundCardRuntimeWorkerPoll::Pending),
        "in-flight disabled result must not be adopted"
    );
    worker.set_enabled(true);
    let resumed_rate = SampleRate::new(96_000).unwrap();
    let resumed = worker.request_start_for_driver(resumed_rate, driver()).unwrap();
    assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), resumed_rate);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let SoundCardRuntimeWorkerPoll::Started(started) = worker.poll() {
            assert_eq!(started.request, resumed);
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    drop(worker);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn idle_selection_releases_the_slot_while_disabled_even_after_startup_identity_exhaustion() {
    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, |_, _| {
        panic!("idle selection must not construct a native endpoint")
    })
    .unwrap();
    worker.set_enabled(false);
    worker.next_request_id = u64::MAX;
    assert!(matches!(
        worker.request_start_for_driver(rate(), driver()),
        Err(StartRequestError::Invalid(_))
    ));
    worker.request_start_for_driver(rate(), None).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert!(matches!(worker.poll(), SoundCardRuntimeWorkerPoll::Disconnected));
    let next = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, |_, _| Err("unused".into())).unwrap();
    drop(next);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn idle_selection_after_a_blocked_constructor_retires_without_a_stale_startup_result() {
    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, move |_, _| {
        started_tx.send(()).unwrap();
        let _ = release_rx.recv();
        Err("stale outcome".into())
    })
    .unwrap();
    worker.request_start_for_driver(rate(), driver()).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    worker.set_enabled(false);
    worker.request_start_for_driver(rate(), None).unwrap();
    assert!(!pool.wait_for_idle(Duration::ZERO));
    assert!(pool.try_reserve().is_err());
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert!(matches!(worker.poll(), SoundCardRuntimeWorkerPoll::Disconnected));
    assert!(started_rx.try_recv().is_err());
}

#[test]
fn idle_selection_retains_the_lifecycle_slot_until_blocked_native_cleanup_finishes() {
    use super::super::events::SoundCardRuntimeEvents;
    use std::sync::Mutex;

    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let native_lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let worker_native_lifetimes = native_lifetimes.clone();
    let (cleanup_tx, cleanup_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(Some(release_rx));
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, move |rate, driver| {
        let mut runtime = SoundCardRuntime::start_selected(rate, driver)?;
        let mut native = golden_audio::AudioEngineBuilder::default()
            .without_backends()
            .with_backend(golden_audio::NullBackend)
            .build()
            .unwrap();
        let events = native.take_event_receiver().unwrap();
        let control = native.control();
        let cleanup_tx = cleanup_tx.clone();
        let release = release_rx.lock().unwrap().take().expect("one constructor");
        runtime.event_bridge = SoundCardRuntimeEvents::spawn_using(
            native,
            events,
            worker_native_lifetimes.try_reserve().unwrap(),
            60,
            move |mut native| {
                cleanup_tx.send(()).unwrap();
                let _ = release.recv();
                let _ = native.shutdown();
            },
        )?;
        runtime.control = control;
        Ok(runtime)
    })
    .unwrap();
    worker.request_start_for_driver(rate(), driver()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let runtime = loop {
        if let SoundCardRuntimeWorkerPoll::Started(started) = worker.poll() {
            break started.result.unwrap();
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    };
    worker.set_enabled(false);
    worker.request_start_for_driver(rate(), None).unwrap();
    drop(runtime);
    cleanup_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(!pool.wait_for_idle(Duration::ZERO));
    assert!(pool.try_reserve().is_err());
    assert!(matches!(worker.poll(), SoundCardRuntimeWorkerPoll::Pending));
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert!(native_lifetimes.wait_for_idle(Duration::from_secs(3)));
    assert!(matches!(worker.poll(), SoundCardRuntimeWorkerPoll::Disconnected));
}

#[test]
fn a_newer_driver_selection_supersedes_an_idle_request_before_retirement() {
    let engine = Engine::new(Folder::new("root"));
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = AtomicBool::new(true);
    let mut worker = SoundCardRuntimeWorker::spawn_using(wake(&engine), &pool, move |rate, _| {
        started_tx.send(rate).unwrap();
        if first.swap(false, Ordering::Relaxed) {
            let _ = release_rx.recv();
        }
        Err("injected outcome".into())
    })
    .unwrap();
    let initial = worker.request_start_for_driver(rate(), driver()).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    worker.request_start_for_driver(rate(), None).unwrap();
    let replacement_rate = SampleRate::new(96_000).unwrap();
    let replacement = worker.request_start_for_driver(replacement_rate, driver()).unwrap();
    release_tx.send(()).unwrap();
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
        replacement_rate
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut requests = Vec::new();
    while requests.len() < 2 {
        if let SoundCardRuntimeWorkerPoll::Started(started) = worker.poll() {
            requests.push(started.request);
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(requests, vec![initial, replacement]);
    worker.request_start_for_driver(rate(), None).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}
