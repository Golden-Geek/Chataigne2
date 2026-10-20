use super::*;
use std::{sync::mpsc, time::Duration};

#[test]
fn native_cleanup_stays_off_consumer_and_holds_capacity_until_engine_destruction_finishes() {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let mut audio = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .build()
        .unwrap();
    let events = audio.take_event_receiver().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let bridge = SoundCardRuntimeEvents::spawn_using(
        audio,
        events,
        lifetimes.try_reserve().unwrap(),
        60,
        move |mut engine| {
            started_tx.send(thread::current().id()).unwrap();
            let _ = release_rx.recv();
            let _ = engine.shutdown();
        },
    )
    .unwrap();
    let completion = bridge.completion();
    bridge.stop();
    let cleanup_thread = started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_ne!(cleanup_thread, thread::current().id());
    assert!(bridge.is_disconnected(), "exit notification precedes blocked cleanup");
    assert!(lifetimes.try_reserve().is_err());
    assert!(!completion.wait_for_idle(Duration::ZERO));
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let consumer = thread::spawn(move || {
        drop(bridge);
        dropped_tx.send(()).unwrap();
    });
    let nonblocking = dropped_rx.recv_timeout(Duration::from_secs(1));
    release_tx.send(()).unwrap();
    assert!(
        nonblocking.is_ok(),
        "consumer drop must return before native cleanup is released"
    );
    consumer.join().unwrap();
    assert!(completion.wait_for_idle(Duration::from_secs(3)));
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn producer_exit_preserves_its_published_prefix_until_the_consumer_drains_it() {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let mut audio = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .build()
        .unwrap();
    let control = audio.control();
    let events = audio.take_event_receiver().unwrap();
    let bridge = SoundCardRuntimeEvents::spawn(audio, events, lifetimes.try_reserve().unwrap(), 60).unwrap();
    control.shutdown().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !bridge.is_disconnected() {
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(bridge.has_pending());
    assert!(bridge
        .drain()
        .iter()
        .any(|event| matches!(event, AudioEvent::ShutdownComplete)));
    assert!(!bridge.has_pending());
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}
