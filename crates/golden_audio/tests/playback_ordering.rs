#![cfg(feature = "playback")]

use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

#[cfg(feature = "analysis")]
use allocation_counter::measure;
#[cfg(feature = "analysis")]
use golden_audio::{
    AudioBackend, AudioCallbackTimestamp, AudioChannelId, AudioConfiguration, AudioDeviceReadiness,
    AudioDeviceSelection, AudioDirection, AudioError, AudioErrorCategory, AudioInspectorError, AudioRouteId,
    AudioStream, AudioStreamHandler, AudioStreamStatus, ConfigGeneration, DirectionConfiguration, GainDb,
    InputPatchRoute, InterleavedInput, InterleavedOutput, MonitorRoute, NullBackend, OutputPatchRoute,
    PhysicalChannelKey, PlaybackRoute, StreamRequest, VirtualInputChannel, VirtualOutputChannel,
};
use golden_audio::{
    AudioCommand, AudioEngine, AudioEngineBuilder, AudioEvent, PlanarBuffer, PlayFileRequest, PlaybackCommandIgnored,
    PlaybackCommandIgnoredReason, PlaybackCommandKind, PlaybackId, PlaybackObservation, PlaybackStopReason,
};
#[cfg(feature = "analysis")]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
};
use tempfile::{Builder, NamedTempFile};

mod playback_ordering_cases;

#[cfg(feature = "analysis")]
#[test]
fn managed_runtime_drives_playback_and_the_backend_output_callback() {
    let callback_signal = Arc::new(AtomicBool::new(false));
    let callback_count = Arc::new(AtomicU64::new(0));
    let input_callback_allocation = Arc::new(AtomicU64::new(u64::MAX));
    let output_callback_allocation = Arc::new(AtomicU64::new(u64::MAX));
    let runtime_failure = Arc::new(AtomicBool::new(false));
    let opened_streams = Arc::new(AtomicU64::new(0));
    let backend = CallbackBackend {
        callback_signal: Arc::clone(&callback_signal),
        callback_count: Arc::clone(&callback_count),
        input_callback_allocation: Arc::clone(&input_callback_allocation),
        output_callback_allocation: Arc::clone(&output_callback_allocation),
        runtime_failure,
        opened_streams,
        continuity: None,
    };
    let file = sine_wave_file(1, 48_000, 4_096);
    let playback_id = PlaybackId::new("managed").unwrap();
    let mut engine = AudioEngineBuilder::default()
        .with_backend(backend)
        .with_managed_render_runtime()
        .build()
        .unwrap();
    assert!(engine.take_playback_renderer().is_none());
    let events = engine.take_event_receiver().unwrap();
    let output = AudioChannelId::new();
    let mut configuration = AudioConfiguration::empty();
    configuration.physical_outputs = null_physical_channels("output");
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Output,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    configuration.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Managed output".to_owned(),
        gain: GainDb::UNITY,
    });
    configuration.playback_patch.push(PlaybackRoute {
        id: AudioRouteId::new(),
        source_channel: 0,
        destination: output,
        gain: GainDb::UNITY,
    });
    for destination in ["output:0", "output:1"] {
        configuration.output_patch.push(OutputPatchRoute {
            id: AudioRouteId::new(),
            source: output,
            destination: PhysicalChannelKey::new(destination).unwrap(),
            gain: GainDb::UNITY,
        });
    }
    let generation = ConfigGeneration::new(1);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration),
        })
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::ConfigurationApplied { generation: applied } if *applied == generation),
    );

    engine
        .control()
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            file.path(),
            playback_id.clone(),
        )))
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackFinished(info) if info.playback_id == playback_id),
    );
    wait_until(|| callback_signal.load(Ordering::Relaxed));
    wait_until(|| output_callback_allocation.load(Ordering::Relaxed) != u64::MAX);
    let observation = engine.observations().latest();
    assert!(observation.runtime.rendered_frames >= 4_096);
    assert!(callback_count.load(Ordering::Relaxed) > 0);
    assert_eq!(output_callback_allocation.load(Ordering::Relaxed), 0);
    assert_eq!(observation.playback.active_voices, 0);
    engine.shutdown().unwrap();
}

#[cfg(feature = "analysis")]
#[test]
fn managed_streaming_continues_while_the_host_thread_is_stalled() {
    let callback_signal = Arc::new(AtomicBool::new(false));
    let callback_count = Arc::new(AtomicU64::new(0));
    let continuity = Arc::new(OutputContinuity::default());
    let backend = CallbackBackend {
        callback_signal,
        callback_count: Arc::clone(&callback_count),
        input_callback_allocation: Arc::new(AtomicU64::new(u64::MAX)),
        output_callback_allocation: Arc::new(AtomicU64::new(u64::MAX)),
        runtime_failure: Arc::new(AtomicBool::new(false)),
        opened_streams: Arc::new(AtomicU64::new(0)),
        continuity: Some(Arc::clone(&continuity)),
    };
    let file = sine_wave_file(2, 48_000, 96_000);
    let playback_id = PlaybackId::new("host-stall-stream").unwrap();
    let mut builder = AudioEngineBuilder::default();
    builder.limits.resident_asset_threshold_bytes = 1;
    builder.limits.stream_ring_frames = 1_024;
    let mut engine = builder
        .with_backend(backend)
        .with_managed_render_runtime()
        .build()
        .unwrap();
    let events = engine.take_event_receiver().unwrap();
    let output = AudioChannelId::new();
    let mut configuration = AudioConfiguration::empty();
    configuration.physical_outputs = null_physical_channels("output");
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Output,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    configuration.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Host-independent output".to_owned(),
        gain: GainDb::UNITY,
    });
    configuration.playback_patch.push(PlaybackRoute {
        id: AudioRouteId::new(),
        source_channel: 0,
        destination: output,
        gain: GainDb::UNITY,
    });
    for destination in ["output:0", "output:1"] {
        configuration.output_patch.push(OutputPatchRoute {
            id: AudioRouteId::new(),
            source: output,
            destination: PhysicalChannelKey::new(destination).unwrap(),
            gain: GainDb::UNITY,
        });
    }
    let generation = ConfigGeneration::new(1);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration),
        })
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::ConfigurationApplied { generation: applied } if *applied == generation),
    );
    engine
        .control()
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            file.path(),
            playback_id.clone(),
        )))
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    wait_until(|| continuity.observed_signal.load(Ordering::Acquire));

    let callbacks_before_stall = callback_count.load(Ordering::Acquire);
    continuity.monitor.store(true, Ordering::Release);
    let stall_deadline = Instant::now() + Duration::from_millis(350);
    while Instant::now() < stall_deadline {
        std::hint::spin_loop();
    }
    continuity.monitor.store(false, Ordering::Release);

    assert!(
        callback_count.load(Ordering::Acquire) >= callbacks_before_stall + 50,
        "the device callback stopped progressing with the host thread"
    );
    assert_eq!(
        continuity.silent_callbacks.load(Ordering::Acquire),
        0,
        "streaming output dropped to silence while the host thread was stalled"
    );
    assert_eq!(
        engine.observations().latest().runtime.output_underflow_count,
        0,
        "the managed output bridge underflowed while the host thread was stalled"
    );
    engine.shutdown().unwrap();
}

#[cfg(feature = "analysis")]
#[test]
fn managed_runtime_bridges_backend_input_into_monitoring_and_metering() {
    let callback_signal = Arc::new(AtomicBool::new(false));
    let callback_count = Arc::new(AtomicU64::new(0));
    let input_callback_allocation = Arc::new(AtomicU64::new(u64::MAX));
    let output_callback_allocation = Arc::new(AtomicU64::new(u64::MAX));
    let runtime_failure = Arc::new(AtomicBool::new(false));
    let opened_streams = Arc::new(AtomicU64::new(0));
    let backend = CallbackBackend {
        callback_signal: Arc::clone(&callback_signal),
        callback_count: Arc::clone(&callback_count),
        input_callback_allocation: Arc::clone(&input_callback_allocation),
        output_callback_allocation: Arc::clone(&output_callback_allocation),
        runtime_failure,
        opened_streams,
        continuity: None,
    };
    let mut engine = AudioEngineBuilder::default()
        .with_backend(backend)
        .with_managed_render_runtime()
        .build()
        .unwrap();
    let events = engine.take_event_receiver().unwrap();
    let inputs = [AudioChannelId::new(), AudioChannelId::new()];
    let output = AudioChannelId::new();
    let mut configuration = AudioConfiguration::empty();
    configuration.physical_inputs = null_physical_channels("input");
    configuration.physical_outputs = null_physical_channels("output");
    configuration.input = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Input,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Output,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    for (index, input) in inputs.into_iter().enumerate() {
        configuration.virtual_inputs.push(VirtualInputChannel {
            id: input,
            label: format!("Managed input {}", index + 1),
        });
        configuration.input_patch.push(InputPatchRoute {
            id: AudioRouteId::new(),
            source: PhysicalChannelKey::new(format!("input:{index}")).unwrap(),
            destination: input,
            gain: GainDb::UNITY,
        });
    }
    configuration.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Managed output".to_owned(),
        gain: GainDb::UNITY,
    });
    configuration.monitoring.push(MonitorRoute {
        id: AudioRouteId::new(),
        source: inputs[0],
        destination: output,
        gain: GainDb::UNITY,
    });
    for destination in ["output:0", "output:1"] {
        configuration.output_patch.push(OutputPatchRoute {
            id: AudioRouteId::new(),
            source: output,
            destination: PhysicalChannelKey::new(destination).unwrap(),
            gain: GainDb::UNITY,
        });
    }
    let generation = ConfigGeneration::new(1);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration),
        })
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::ConfigurationApplied { generation: applied } if *applied == generation),
    );
    wait_until(|| callback_signal.load(Ordering::Relaxed));
    wait_until(|| engine.observations().latest().input_global_max_rms > 0.01);
    wait_until(|| input_callback_allocation.load(Ordering::Relaxed) != u64::MAX);
    wait_until(|| output_callback_allocation.load(Ordering::Relaxed) != u64::MAX);

    let observation = engine.observations().latest();
    assert!(observation.output_global_max_rms > 0.01);
    assert!(observation.runtime.rendered_frames > 0);
    assert!(callback_count.load(Ordering::Relaxed) > 0);
    assert_eq!(input_callback_allocation.load(Ordering::Relaxed), 0);
    assert_eq!(output_callback_allocation.load(Ordering::Relaxed), 0);
    engine.shutdown().unwrap();
}

#[cfg(feature = "analysis")]
#[test]
fn managed_runtime_uses_null_clock_without_false_callback_xruns() {
    let mut engine = AudioEngineBuilder::default()
        .with_managed_render_runtime()
        .build()
        .unwrap();
    let events = engine.take_event_receiver().unwrap();
    let mut configuration = AudioConfiguration::empty();
    configuration.physical_inputs = null_physical_channels("input");
    configuration.physical_outputs = null_physical_channels("output");
    configuration.input = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Input,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Output,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    let generation = ConfigGeneration::new(1);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration),
        })
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::ConfigurationApplied { generation: applied } if *applied == generation),
    );
    wait_until(|| engine.observations().latest().runtime.rendered_blocks >= 8);

    let observation = engine.observations().latest();
    assert_eq!(observation.runtime.xrun_count, 0);
    assert_eq!(observation.runtime.input_underflow_count, 0);
    assert_eq!(observation.runtime.output_underflow_count, 0);
    engine.shutdown().unwrap();
}

#[cfg(feature = "analysis")]
#[test]
fn managed_runtime_reopens_a_callback_stream_after_runtime_invalidation() {
    let callback_signal = Arc::new(AtomicBool::new(false));
    let callback_count = Arc::new(AtomicU64::new(0));
    let runtime_failure = Arc::new(AtomicBool::new(false));
    let opened_streams = Arc::new(AtomicU64::new(0));
    let backend = CallbackBackend {
        callback_signal,
        callback_count: Arc::clone(&callback_count),
        input_callback_allocation: Arc::new(AtomicU64::new(u64::MAX)),
        output_callback_allocation: Arc::new(AtomicU64::new(u64::MAX)),
        runtime_failure: Arc::clone(&runtime_failure),
        opened_streams: Arc::clone(&opened_streams),
        continuity: None,
    };
    let mut engine = AudioEngineBuilder::default()
        .with_backend(backend)
        .with_managed_render_runtime()
        .build()
        .unwrap();
    let events = engine.take_event_receiver().unwrap();
    let output = AudioChannelId::new();
    let mut configuration = AudioConfiguration::empty();
    configuration.physical_outputs = null_physical_channels("output");
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Output,
        )),
        recovery_policy: golden_audio::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: golden_audio::AudioBufferPolicy::Fixed(128),
    };
    configuration.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Recovering output".to_owned(),
        gain: GainDb::UNITY,
    });
    for destination in ["output:0", "output:1"] {
        configuration.output_patch.push(OutputPatchRoute {
            id: AudioRouteId::new(),
            source: output,
            destination: PhysicalChannelKey::new(destination).unwrap(),
            gain: GainDb::UNITY,
        });
    }
    let generation = ConfigGeneration::new(1);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration),
        })
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::ConfigurationApplied { generation: applied } if *applied == generation),
    );
    wait_until(|| opened_streams.load(Ordering::Acquire) == 1 && callback_count.load(Ordering::Acquire) > 2);

    runtime_failure.store(true, Ordering::Release);

    wait_until(|| {
        opened_streams.load(Ordering::Acquire) >= 2
            && !runtime_failure.load(Ordering::Acquire)
            && engine.observations().latest().device.output.readiness == AudioDeviceReadiness::Ready
    });
    engine.shutdown().unwrap();
}

#[test]
fn public_engine_runs_ordered_async_playback_through_the_callback_renderer() {
    let file = sine_wave_file(2, 48_000, 256);
    let playback_id = PlaybackId::new("ordered").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let mut renderer = engine.take_playback_renderer().unwrap();
    let events = engine.take_event_receiver().unwrap();
    engine
        .control()
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            file.path(),
            playback_id.clone(),
        )))
        .unwrap();
    let started = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    let voice = match started {
        AudioEvent::PlaybackStarted(info) => info.voice,
        _ => unreachable!(),
    };
    let playback = wait_playback(&engine, |playback| playback.active_voices == 1);
    assert_eq!(playback.active_voices, 1);
    assert_eq!(playback.loading_voices, 0);
    assert_eq!(playback.cache_entries, 1);
    assert!(playback.resident_bytes > 0);

    let mut destination = PlanarBuffer::new(256, 128).unwrap();
    renderer.render(&mut destination, 128).unwrap();
    renderer.render(&mut destination, 128).unwrap();
    let finished = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackFinished(info) if info.voice == voice),
    );
    assert!(matches!(finished, AudioEvent::PlaybackFinished(_)));
    assert_eq!(
        wait_playback(&engine, |playback| playback.active_voices == 0).active_voices,
        0
    );
    engine.shutdown().unwrap();
}

#[test]
fn same_id_replacement_and_pending_stop_never_publish_a_stale_start() {
    let first = sine_wave_file(1, 48_000, 32_768);
    let second = sine_wave_file(2, 48_000, 128);
    let playback_id = PlaybackId::new("replace").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            first.path(),
            playback_id.clone(),
        )))
        .unwrap();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            second.path(),
            playback_id.clone(),
        )))
        .unwrap();

    let started = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    match started {
        AudioEvent::PlaybackStarted(info) => assert_eq!(
            std::fs::canonicalize(info.path).unwrap(),
            std::fs::canonicalize(second.path()).unwrap()
        ),
        _ => unreachable!(),
    }

    let pending_id = PlaybackId::new("pending-stop").unwrap();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            first.path(),
            pending_id.clone(),
        )))
        .unwrap();
    control
        .submit(AudioCommand::StopFile {
            playback_id: pending_id.clone(),
        })
        .unwrap();
    let stopped = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStopped(info)
                if info.playback_id == pending_id && info.voice.is_none()
        )
    });
    assert!(matches!(
        stopped,
        AudioEvent::PlaybackStopped(info) if info.reason == PlaybackStopReason::Requested
    ));
    engine.shutdown().unwrap();
}

#[test]
fn replacement_while_playing_fades_old_voice_and_starts_new_generation() {
    let first = sine_wave_file(1, 48_000, 4_096);
    let second = sine_wave_file(1, 48_000, 4_096);
    let playback_id = PlaybackId::new("playing-replace").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let mut renderer = engine.take_playback_renderer().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            first.path(),
            playback_id.clone(),
        )))
        .unwrap();
    let first_started = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    let first_voice = match first_started {
        AudioEvent::PlaybackStarted(info) => info.voice,
        _ => unreachable!(),
    };
    let mut destination = PlanarBuffer::new(256, 128).unwrap();
    renderer.render(&mut destination, 128).unwrap();

    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            second.path(),
            playback_id.clone(),
        )))
        .unwrap();
    let second_started = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStarted(info)
                if info.playback_id == playback_id && info.voice != first_voice
        )
    });
    assert!(matches!(second_started, AudioEvent::PlaybackStarted(_)));
    for _ in 0..4 {
        renderer.render(&mut destination, 128).unwrap();
    }
    let stopped = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStopped(info)
                if info.voice == Some(first_voice) && info.reason == PlaybackStopReason::Replaced
        )
    });
    assert!(matches!(stopped, AudioEvent::PlaybackStopped(_)));
    engine.shutdown().unwrap();
}

fn wait_event(events: &golden_audio::AudioEventReceiver, predicate: impl Fn(&AudioEvent) -> bool) -> AudioEvent {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        while let Some(event) = events.try_recv() {
            if predicate(&event) {
                return event;
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting for playback event");
        thread::sleep(Duration::from_millis(2));
    }
}

fn wait_playback(engine: &AudioEngine, predicate: impl Fn(PlaybackObservation) -> bool) -> PlaybackObservation {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let playback = engine.observations().latest().playback;
        if predicate(playback) {
            return playback;
        }
        assert!(Instant::now() < deadline, "timed out waiting for playback observation");
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(feature = "analysis")]
fn wait_until(predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for managed callback output"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(feature = "analysis")]
fn null_physical_channels(prefix: &str) -> Vec<PhysicalChannelKey> {
    (0..2)
        .map(|index| PhysicalChannelKey::new(format!("{prefix}:{index}")).unwrap())
        .collect()
}

#[cfg(feature = "analysis")]
#[derive(Clone, Debug)]
struct CallbackBackend {
    callback_signal: Arc<AtomicBool>,
    callback_count: Arc<AtomicU64>,
    input_callback_allocation: Arc<AtomicU64>,
    output_callback_allocation: Arc<AtomicU64>,
    runtime_failure: Arc<AtomicBool>,
    opened_streams: Arc<AtomicU64>,
    continuity: Option<Arc<OutputContinuity>>,
}

#[cfg(feature = "analysis")]
#[derive(Debug, Default)]
struct OutputContinuity {
    observed_signal: AtomicBool,
    monitor: AtomicBool,
    silent_callbacks: AtomicU64,
}

#[cfg(feature = "analysis")]
impl OutputContinuity {
    fn observe(&self, samples: &[f32]) {
        if samples.iter().any(|sample| sample.abs() > 0.001) {
            self.observed_signal.store(true, Ordering::Release);
        } else if self.observed_signal.load(Ordering::Acquire) && self.monitor.load(Ordering::Acquire) {
            self.silent_callbacks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(feature = "analysis")]
impl AudioBackend for CallbackBackend {
    fn id(&self) -> golden_audio::BackendId {
        NullBackend::backend_id()
    }

    fn descriptor(&self) -> golden_audio::BackendDescriptor {
        NullBackend.descriptor()
    }

    fn device_inventory(&self) -> Result<golden_audio::AudioDeviceInventory, AudioError> {
        NullBackend.device_inventory()
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        NullBackend.open_stream(request)
    }

    fn supports_stream_handlers(&self) -> bool {
        true
    }

    fn open_stream_with_handler(
        &self,
        request: &StreamRequest,
        handler: Box<dyn AudioStreamHandler>,
    ) -> Result<Box<dyn AudioStream>, AudioError> {
        self.runtime_failure.store(false, Ordering::Release);
        self.opened_streams.fetch_add(1, Ordering::Relaxed);
        let status = NullBackend.open_stream(request)?.status();
        Ok(Box::new(CallbackStream {
            status,
            direction: request.direction,
            channels: usize::from(request.channels),
            handler: Some(handler),
            running: Arc::new(AtomicBool::new(false)),
            callback_signal: Arc::clone(&self.callback_signal),
            callback_count: Arc::clone(&self.callback_count),
            input_callback_allocation: Arc::clone(&self.input_callback_allocation),
            output_callback_allocation: Arc::clone(&self.output_callback_allocation),
            runtime_failure: Arc::clone(&self.runtime_failure),
            continuity: self.continuity.as_ref().map(Arc::clone),
            worker: None,
        }))
    }
}

#[cfg(feature = "analysis")]
#[derive(Debug)]
struct CallbackStream {
    status: AudioStreamStatus,
    direction: AudioDirection,
    channels: usize,
    handler: Option<Box<dyn AudioStreamHandler>>,
    running: Arc<AtomicBool>,
    callback_signal: Arc<AtomicBool>,
    callback_count: Arc<AtomicU64>,
    input_callback_allocation: Arc<AtomicU64>,
    output_callback_allocation: Arc<AtomicU64>,
    runtime_failure: Arc<AtomicBool>,
    continuity: Option<Arc<OutputContinuity>>,
    worker: Option<JoinHandle<Box<dyn AudioStreamHandler>>>,
}

#[cfg(feature = "analysis")]
impl AudioStream for CallbackStream {
    fn status(&self) -> AudioStreamStatus {
        let mut status = self.status.clone();
        if self.runtime_failure.load(Ordering::Acquire) {
            status.readiness = AudioDeviceReadiness::Recovering;
            status.error = Some(AudioInspectorError {
                category: AudioErrorCategory::StreamNegotiationFailed,
                message: "injected callback stream invalidation".to_owned(),
                technical_detail: None,
            });
        }
        status
    }

    fn start(&mut self) -> Result<(), AudioError> {
        let Some(mut handler) = self.handler.take() else {
            return Err(AudioError::invalid_configuration(
                "callback stream cannot be started twice",
            ));
        };
        self.running.store(true, Ordering::Release);
        let running = Arc::clone(&self.running);
        let callback_signal = Arc::clone(&self.callback_signal);
        let callback_count = Arc::clone(&self.callback_count);
        let input_callback_allocation = Arc::clone(&self.input_callback_allocation);
        let output_callback_allocation = Arc::clone(&self.output_callback_allocation);
        let continuity = self.continuity.as_ref().map(Arc::clone);
        let direction = self.direction;
        let channels = self.channels;
        self.worker = Some(thread::spawn(move || {
            let mut samples = vec![0.0_f32; channels * 128];
            let mut callback_nanos = 0_u128;
            let mut phase = 0.0_f32;
            while running.load(Ordering::Acquire) {
                let timestamp = AudioCallbackTimestamp {
                    callback_nanos,
                    device_nanos: callback_nanos,
                };
                match direction {
                    AudioDirection::Input => {
                        for frame in 0..128 {
                            let sample = phase.sin() * 0.25;
                            phase += 440.0 * std::f32::consts::TAU / 48_000.0;
                            for channel in 0..channels {
                                samples[frame * channels + channel] = sample;
                            }
                        }
                        if input_callback_allocation.load(Ordering::Relaxed) == u64::MAX && callback_nanos != 0 {
                            let allocation =
                                measure(|| handler.process_input(InterleavedInput::F32(&samples), timestamp));
                            input_callback_allocation.store(
                                u64::from(
                                    allocation.count_total != 0
                                        || allocation.count_current != 0
                                        || allocation.bytes_total != 0
                                        || allocation.bytes_current != 0,
                                ),
                                Ordering::Relaxed,
                            );
                        } else {
                            handler.process_input(InterleavedInput::F32(&samples), timestamp);
                        }
                    }
                    AudioDirection::Output => {
                        if output_callback_allocation.load(Ordering::Relaxed) == u64::MAX && callback_nanos != 0 {
                            let allocation =
                                measure(|| handler.process_output(InterleavedOutput::F32(&mut samples), timestamp));
                            output_callback_allocation.store(
                                u64::from(
                                    allocation.count_total != 0
                                        || allocation.count_current != 0
                                        || allocation.bytes_total != 0
                                        || allocation.bytes_current != 0,
                                ),
                                Ordering::Relaxed,
                            );
                        } else {
                            handler.process_output(InterleavedOutput::F32(&mut samples), timestamp);
                        }
                        if samples.iter().any(|sample| sample.abs() > 0.001) {
                            callback_signal.store(true, Ordering::Relaxed);
                        }
                        if let Some(continuity) = &continuity {
                            continuity.observe(&samples);
                        }
                    }
                }
                callback_count.fetch_add(1, Ordering::Relaxed);
                callback_nanos = callback_nanos.saturating_add(2_666_667);
                thread::sleep(Duration::from_millis(2));
            }
            handler
        }));
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.running.store(false, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            self.handler = Some(
                worker
                    .join()
                    .map_err(|_| AudioError::invalid_configuration("callback stream thread panicked"))?,
            );
        }
        Ok(())
    }
}

fn sine_wave_file(channels: u16, sample_rate: u32, frames: u32) -> NamedTempFile {
    let mut file = Builder::new().suffix(".wav").tempfile().unwrap();
    write_sine_wave(file.as_file_mut(), channels, sample_rate, frames);
    file
}

fn write_sine_wave(writer: &mut impl Write, channels: u16, sample_rate: u32, frames: u32) {
    let data_bytes = frames.saturating_mul(u32::from(channels)).saturating_mul(2);
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
    writer.write_all(b"WAVEfmt ").unwrap();
    writer.write_all(&16_u32.to_le_bytes()).unwrap();
    writer.write_all(&1_u16.to_le_bytes()).unwrap();
    writer.write_all(&channels.to_le_bytes()).unwrap();
    writer.write_all(&sample_rate.to_le_bytes()).unwrap();
    writer
        .write_all(&(sample_rate * u32::from(channels) * 2).to_le_bytes())
        .unwrap();
    writer.write_all(&(channels * 2).to_le_bytes()).unwrap();
    writer.write_all(&16_u16.to_le_bytes()).unwrap();
    writer.write_all(b"data").unwrap();
    writer.write_all(&data_bytes.to_le_bytes()).unwrap();
    for frame in 0..frames {
        let sample = (((frame as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32).sin()) * 12_000.0) as i16;
        for _ in 0..channels {
            writer.write_all(&sample.to_le_bytes()).unwrap();
        }
    }
    writer.flush().unwrap();
}
