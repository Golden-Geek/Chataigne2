use super::*;
use golden_audio::AudioEvent;
use std::{sync::mpsc, time::Duration};

struct PressureFixture {
    bridge: SoundCardRuntimeEvents,
    control: golden_audio::AudioControl,
    observations: golden_audio::AudioObservationReader,
    lifetimes: RetirementPool,
    generations: Vec<golden_audio::ConfigGeneration>,
}

fn wait_for_pressure(bridge: &SoundCardRuntimeEvents) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while bridge.pending.metrics().rejected == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "fixture must exercise a full downstream queue"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert!(bridge.pending.metrics().depth <= EVENT_ITEMS);
    let sequence = bridge.observation().sequence;
    while bridge.observation().sequence == sequence {
        assert!(
            std::time::Instant::now() < deadline,
            "full event mailbox must not freeze native observations"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn pressured_bridge() -> PressureFixture {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let mut builder = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .with_ordered_events();
    builder.limits.event_queue_capacity = 1;
    builder.limits.command_queue_capacity = 256;
    let mut audio = builder.build().unwrap();
    let control = audio.control();
    let observations = audio.observations();
    let events = audio.take_event_receiver().unwrap();
    let bridge = SoundCardRuntimeEvents::spawn(audio, events, lifetimes.try_reserve().unwrap(), 60).unwrap();
    let mut generation = golden_audio::ConfigGeneration::INITIAL;
    let mut generations = Vec::new();
    for _ in 0..80 {
        generation = generation.next();
        control
            .submit(golden_audio::AudioCommand::ApplyConfiguration {
                generation,
                config: Box::default(),
            })
            .unwrap();
        generations.push(generation);
    }
    PressureFixture {
        bridge,
        control,
        observations,
        lifetimes,
        generations,
    }
}

#[test]
fn large_playback_occurrences_hit_byte_pressure_before_the_app_item_limit() {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let mut builder = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .with_ordered_events();
    builder.limits.event_queue_capacity = 1;
    builder.limits.command_queue_capacity = 256;
    let mut audio = builder.build().unwrap();
    let control = audio.control();
    let observations = audio.observations();
    let events = audio.take_event_receiver().unwrap();
    let bridge = SoundCardRuntimeEvents::spawn(audio, events, lifetimes.try_reserve().unwrap(), 60).unwrap();
    let id = golden_audio::PlaybackId::new("p".repeat(golden_audio::PlaybackId::MAX_BYTES)).unwrap();
    // An overlong single filename fails native file lookup; no fixture or physical
    // audio endpoint is opened. Both ignored and failed outcomes retain the path.
    let path = std::path::PathBuf::from("x".repeat(golden_audio::PlayFileRequest::MAX_PATH_BYTES));
    for _ in 0..80 {
        control
            .submit(golden_audio::AudioCommand::PlayFile(
                golden_audio::PlayFileRequest::new(path.clone(), id.clone()).with_force_restart(false),
            ))
            .unwrap();
    }
    wait_for_pressure(&bridge);
    let metrics = bridge.pending.metrics();
    assert!(
        metrics.depth < EVENT_ITEMS,
        "byte capacity must fill before 64 large occurrences"
    );
    assert!(metrics.retained_weight <= EVENT_BYTES);
    assert_eq!(observations.latest().dropped_event_count, 0);
    control.shutdown().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut outcomes = 0;
    let mut terminal = false;
    while !bridge.is_disconnected() || bridge.has_pending() {
        let events = bridge.drain();
        assert!(events.len() <= EVENT_SERVICE_ITEMS);
        for event in events {
            match &event.event {
                AudioEvent::PlaybackFailed(failure) => {
                    assert_eq!(failure.playback_id, id);
                    assert_eq!(failure.path, path);
                    outcomes += 1;
                }
                AudioEvent::PlaybackCommandIgnored(golden_audio::PlaybackCommandIgnored::PlayFileAlreadyActive {
                    playback_id,
                    path: reported,
                    ..
                }) => {
                    assert_eq!(playback_id, &id);
                    assert_eq!(reported, &path);
                    outcomes += 1;
                }
                AudioEvent::PlaybackStopped(info) => {
                    assert_eq!(info.playback_id, id);
                    outcomes += 1;
                }
                AudioEvent::ShutdownComplete => terminal = true,
                AudioEvent::PlaybackStarted(_) | AudioEvent::PlaybackFinished(_) => {
                    panic!("invalid filename cannot produce playable audio")
                }
                _ => {}
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "large accepted occurrences must drain through shutdown"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(outcomes, 80);
    assert!(terminal);
    assert_eq!(observations.latest().dropped_event_count, 0);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn pressure_preserves_every_configuration_completion_with_bounded_actor_service() {
    let PressureFixture {
        bridge,
        control,
        observations,
        lifetimes,
        generations,
    } = pressured_bridge();
    wait_for_pressure(&bridge);
    assert_eq!(bridge.pending.metrics().depth, EVENT_ITEMS);
    control.shutdown().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut applied = Vec::new();
    let mut terminal = false;
    while !bridge.is_disconnected() || bridge.has_pending() {
        let events = bridge.drain();
        assert!(events.len() <= EVENT_SERVICE_ITEMS);
        for event in events {
            match &event.event {
                AudioEvent::ConfigurationApplied { generation } => applied.push(*generation),
                AudioEvent::ShutdownComplete => terminal = true,
                AudioEvent::ConfigurationRejected { error, .. } => panic!("unexpected rejection: {error}"),
                _ => {}
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "ordered pipeline must keep progressing"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(applied, generations);
    assert!(terminal);
    assert_eq!(observations.latest().dropped_event_count, 0);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn stopping_a_full_pipeline_releases_both_capacity_waits_and_native_lifetime() {
    let PressureFixture { bridge, lifetimes, .. } = pressured_bridge();
    wait_for_pressure(&bridge);
    bridge.stop();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
    assert!(bridge.is_disconnected());
}

#[test]
fn desired_disable_survives_full_control_admission_and_applies_after_pressure_clears() {
    use super::super::SoundCardRuntime;
    use golden_core::{
        engine::EngineTime,
        process_ctx::{ExecutionPhase, ProcessCtx},
    };

    let PressureFixture {
        bridge,
        control,
        observations: _,
        lifetimes,
        ..
    } = pressured_bridge();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    wait_for_pressure(&bridge);
    let mut runtime = SoundCardRuntime::start_selected(
        golden_audio::SampleRate::default(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    // Replace the complete native endpoint with the same real NullBackend stack
    // configured with small queues. Facade behavior stays on the production path.
    runtime.event_bridge = bridge;
    runtime.control = control;
    let mut filled = false;
    for _ in 0..512 {
        if runtime
            .control
            .submit(golden_audio::AudioCommand::SetEnabled(true))
            .is_err()
        {
            filled = true;
            break;
        }
    }
    assert!(filled);
    assert!(runtime.set_enabled(false).is_err());
    assert_eq!(runtime.enabled_pending, Some(false));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );
    loop {
        let (observation, events) = runtime.poll(&mut ctx, golden_core::node::NodeId(1), true);
        assert!(events.len() <= EVENT_SERVICE_ITEMS);
        if runtime.enabled_pending.is_none() && !observation.enabled {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "retained disable must eventually apply"
        );
        thread::sleep(Duration::from_millis(1));
    }
    drop(runtime);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}

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
        .any(|event| matches!(&event.event, AudioEvent::ShutdownComplete)));
    assert!(!bridge.has_pending());
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}

fn preparation_pressure(
    callback_pressure: bool,
) -> (
    SoundCardRuntimeEvents,
    golden_audio::AudioControl,
    RetirementPool,
    golden_io::PayloadReservation,
) {
    let events_pool = golden_io::PayloadPool::new(
        "audio-event-admission-test",
        if callback_pressure { EVENT_ITEMS } else { 1 },
        EVENT_BYTES,
    )
    .unwrap();
    let callbacks_pool = golden_io::PayloadPool::new(
        "audio-callback-admission-test",
        if callback_pressure { 1 } else { EVENT_ITEMS },
        EVENT_BYTES,
    )
    .unwrap();
    let occupied = if callback_pressure {
        callbacks_pool.try_reserve(1)
    } else {
        events_pool.try_reserve(1)
    }
    .unwrap();
    let preparer = EventPreparer::using(events_pool, callbacks_pool);
    let mut audio = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .with_ordered_events()
        .build()
        .unwrap();
    let control = audio.control();
    let events = audio.take_event_receiver().unwrap();
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let bridge = SoundCardRuntimeEvents::spawn_with_preparer(
        audio,
        events,
        lifetimes.try_reserve().unwrap(),
        30,
        move || Ok(preparer),
        |mut audio| {
            let _ = audio.shutdown();
        },
    )
    .unwrap();
    control
        .submit(golden_audio::AudioCommand::PlayFile(
            golden_audio::PlayFileRequest::new(
                "sound-card-admission-missing-file.wav",
                golden_audio::PlaybackId::from_static("admitted"),
            ),
        ))
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while bridge.capacity.preparation_waits.load(Ordering::Acquire) == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "native owner must reach lifetime capacity wait"
        );
        thread::sleep(Duration::from_millis(1));
    }
    (bridge, control, lifetimes, occupied)
}

#[test]
fn native_preparation_waits_recover_both_pools_and_preserve_the_original_outcome() {
    for callback_pressure in [false, true] {
        let (bridge, control, lifetimes, occupied) = preparation_pressure(callback_pressure);
        drop(occupied);
        control.shutdown().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut failed = 0;
        let mut terminal = false;
        while !bridge.is_disconnected() || bridge.has_pending() {
            for event in bridge.drain() {
                assert!(event.preparation_error.is_none());
                let playback_id = match &event.event {
                    AudioEvent::PlaybackFailed(failure) => Some(&failure.playback_id),
                    AudioEvent::PlaybackStopped(info) => Some(&info.playback_id),
                    _ => None,
                };
                if let Some(id) = playback_id {
                    assert_eq!(id.as_str(), "admitted");
                    assert_eq!(
                        event.callback(golden_core::node::NodeId(1)).unwrap().payload["args"][0],
                        "admitted"
                    );
                    failed += 1;
                }
                terminal |= matches!(&event.event, AudioEvent::ShutdownComplete);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "capacity release must resume native delivery"
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(failed, 1);
        assert!(terminal);
        assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
    }
}

#[test]
fn cancellation_wakes_both_native_preparation_capacity_waits_without_joining() {
    for callback_pressure in [false, true] {
        let (bridge, _control, lifetimes, occupied) = preparation_pressure(callback_pressure);
        bridge.stop();
        assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
        assert!(bridge.is_disconnected());
        drop(occupied);
    }
}

#[test]
fn preparation_setup_failure_keeps_the_native_lifetime_until_cleanup_finishes() {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let permit = lifetimes.try_reserve().unwrap();
    let mut audio = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .build()
        .unwrap();
    let events = audio.take_event_receiver().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner = thread::spawn(move || {
        SoundCardRuntimeEvents::spawn_with_preparer(
            audio,
            events,
            permit,
            30,
            || Err("preparation setup fixture".into()),
            move |mut audio| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let _ = audio.shutdown();
            },
        )
    });
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let charged = lifetimes.try_reserve().is_err();
    release_tx.send(()).unwrap();
    assert!(
        charged,
        "fallible setup must not release capacity ahead of native cleanup"
    );
    assert_eq!(owner.join().unwrap().unwrap_err(), "preparation setup fixture");
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}
