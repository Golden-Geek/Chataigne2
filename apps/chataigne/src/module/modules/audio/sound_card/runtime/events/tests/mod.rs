use super::*;
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
    assert_eq!(bridge.pending.metrics().depth, EVENT_ITEMS);
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
fn pressure_preserves_every_configuration_completion_with_bounded_actor_service() {
    let PressureFixture {
        bridge,
        control,
        observations,
        lifetimes,
        generations,
    } = pressured_bridge();
    wait_for_pressure(&bridge);
    control.shutdown().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut applied = Vec::new();
    let mut terminal = false;
    while !bridge.is_disconnected() || bridge.has_pending() {
        let events = bridge.drain();
        assert!(events.len() <= EVENT_SERVICE_ITEMS);
        for event in events {
            match event {
                AudioEvent::ConfigurationApplied { generation } => applied.push(generation),
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
        observations,
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
    runtime.observations = observations;
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
        let (observation, _, events) = runtime.poll(&mut ctx);
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
        .any(|event| matches!(event, AudioEvent::ShutdownComplete)));
    assert!(!bridge.has_pending());
    assert!(lifetimes.wait_for_idle(Duration::from_secs(3)));
}
