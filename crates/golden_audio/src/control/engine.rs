use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicU8, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use rtrb::Consumer;

use crate::{
    AudioBackend, AudioCommand, AudioDeviceInspectorState, AudioEngineConfig, AudioError, AudioErrorCategory,
    AudioEvent, AudioObservationReader, AudioObservationSnapshot, AudioQueueKind, BackendPolicy, CommandSequence,
    DiagnosticEvent, DiagnosticSeverity, EngineLimits, NullBackend, QueuePressureCounters, QueuePressureEvent,
};
#[cfg(feature = "playback")]
use crate::{
    PlaybackScheduler, PlaybackSchedulerConfig, PlaybackStopReason, PlaybackVoiceController, PlaybackVoiceRenderer,
    playback_voice_pool,
};

use super::configuration::{ApplyConfigurationContext, apply_configuration, validate_backends};
use super::device_runtime::DeviceRuntime;
use super::ingress::{CommandEnvelope, CommandQueueProducer, command_queue};
#[cfg(feature = "playback")]
use super::playback::{PlaybackLifecycles, drain_playback_work, schedule_playback, stop_all_playback, stop_playback};
#[cfg(all(feature = "analysis", feature = "playback"))]
use super::render_runtime::ManagedRenderRuntime;

const STATE_RUNNING: u8 = 0;
const STATE_STOPPING: u8 = 1;
const STATE_STOPPED: u8 = 2;

#[derive(Clone, Debug)]
pub struct AudioControl {
    sender: CommandQueueProducer,
    state: Arc<AtomicU8>,
    next_sequence: Arc<AtomicU64>,
}

impl AudioControl {
    pub fn submit(&self, command: AudioCommand) -> Result<CommandSequence, AudioError> {
        if matches!(command, AudioCommand::Shutdown) {
            return self.shutdown();
        }
        if self.state.load(Ordering::Acquire) != STATE_RUNNING {
            return Err(AudioError::shutting_down());
        }
        let sequence = next_sequence(&self.next_sequence)?;
        self.sender.try_push(CommandEnvelope { sequence, command })?;
        Ok(sequence)
    }

    /// Replaces the exact device targets whose capabilities the control worker
    /// should probe. Catalog-only entries remain lightweight until included.
    pub fn set_device_probe_interests(
        &self,
        targets: Vec<crate::AudioDeviceTargetId>,
    ) -> Result<CommandSequence, AudioError> {
        self.submit(AudioCommand::SetDeviceProbeInterests { targets })
    }

    pub fn shutdown(&self) -> Result<CommandSequence, AudioError> {
        match self
            .state
            .compare_exchange(STATE_RUNNING, STATE_STOPPING, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {}
            Err(STATE_STOPPING) | Err(STATE_STOPPED) => {
                return Ok(CommandSequence::FIRST);
            }
            Err(_) => return Err(AudioError::shutting_down()),
        }

        let sequence = next_sequence(&self.next_sequence)?;
        match self.sender.try_push(CommandEnvelope {
            sequence,
            command: AudioCommand::Shutdown,
        }) {
            Ok(()) => Ok(sequence),
            Err(error) => {
                self.state.store(STATE_RUNNING, Ordering::Release);
                Err(error)
            }
        }
    }
}

#[derive(Debug)]
pub struct AudioEventReceiver {
    receiver: Receiver<AudioEvent>,
}

impl AudioEventReceiver {
    #[must_use]
    pub fn try_recv(&self) -> Option<AudioEvent> {
        self.receiver.try_recv().ok()
    }

    #[must_use]
    pub fn drain(&self) -> Vec<AudioEvent> {
        self.receiver.try_iter().collect()
    }

    pub fn recv(&self) -> Result<AudioEvent, AudioError> {
        self.receiver.recv().map_err(|_| AudioError::shutting_down())
    }

    /// Waits for an event until `timeout`, allowing host adapters to combine
    /// event-driven delivery with a deliberately coarse observation cadence.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<AudioEvent>, AudioError> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(AudioError::shutting_down()),
        }
    }
}

#[derive(Debug)]
pub struct AudioEngineBuilder {
    pub config: crate::AudioEngineConfig,
    pub limits: EngineLimits,
    pub backend_policy: BackendPolicy,
    backends: Vec<Arc<dyn AudioBackend>>,
    managed_render_runtime: bool,
}

impl Default for AudioEngineBuilder {
    fn default() -> Self {
        Self {
            config: crate::AudioEngineConfig::default(),
            limits: EngineLimits::default(),
            backend_policy: BackendPolicy::default(),
            backends: vec![Arc::new(NullBackend)],
            managed_render_runtime: false,
        }
    }
}

impl AudioEngineBuilder {
    #[must_use]
    pub fn with_backend(mut self, backend: impl AudioBackend + 'static) -> Self {
        let id = backend.id();
        self.backends.retain(|current| current.id() != id);
        self.backends.push(Arc::new(backend));
        self
    }

    /// Registers an already type-erased backend.
    ///
    /// Native host discovery returns boxed backends because the set of compiled
    /// host implementations is platform-dependent. Keeping that erasure at the
    /// builder boundary lets applications register the complete native set
    /// without knowing concrete backend types.
    #[must_use]
    pub fn with_boxed_backend(mut self, backend: Box<dyn AudioBackend>) -> Self {
        let id = backend.id();
        self.backends.retain(|current| current.id() != id);
        self.backends.push(Arc::from(backend));
        self
    }

    #[must_use]
    pub fn without_backends(mut self) -> Self {
        self.backends.clear();
        self
    }

    /// Lets Golden Audio own the render worker and backend callback bridges.
    ///
    /// This is the ready-to-run path for product adapters. External audio hosts
    /// can keep taking the callback renderer directly.
    #[must_use]
    pub fn with_managed_render_runtime(mut self) -> Self {
        self.managed_render_runtime = true;
        self
    }

    pub fn build(self) -> Result<AudioEngine, AudioError> {
        self.config.validate()?;
        self.limits.validate()?;
        validate_backends(&self.backends, &self.backend_policy)?;
        #[cfg(feature = "playback")]
        let (playback_voice_controller, playback_renderer) = playback_voice_pool(
            self.limits.max_voices,
            self.limits.max_virtual_outputs,
            self.config.internal_block_frames.get() as usize,
        )?;
        #[cfg(all(feature = "analysis", feature = "playback"))]
        let mut playback_renderer = Some(playback_renderer);
        #[cfg(all(feature = "playback", not(feature = "analysis")))]
        let playback_renderer = Some(playback_renderer);
        #[cfg(feature = "playback")]
        let playback_scheduler =
            PlaybackScheduler::new(PlaybackSchedulerConfig::from_engine(&self.config, &self.limits))?;

        let pressure = QueuePressureCounters::default();
        let (command_sender, command_receiver, worker_thread) =
            command_queue(self.limits.command_queue_capacity, pressure.clone());
        let (event_sender, event_receiver) = sync_channel(self.limits.event_queue_capacity);
        let observation = Arc::new(RwLock::new(AudioObservationSnapshot {
            device: AudioDeviceInspectorState {
                engine_sample_rate: self.config.sample_rate.get(),
                ..AudioDeviceInspectorState::default()
            },
            ..AudioObservationSnapshot::default()
        }));
        let state = Arc::new(AtomicU8::new(STATE_RUNNING));
        let next_sequence = Arc::new(AtomicU64::new(CommandSequence::FIRST.get()));
        let control = AudioControl {
            sender: command_sender.clone(),
            state: Arc::clone(&state),
            next_sequence: Arc::clone(&next_sequence),
        };
        let observations = AudioObservationReader {
            shared: Arc::clone(&observation),
        };
        let limits = self.limits;
        let engine_config = self.config;
        #[cfg(all(feature = "analysis", feature = "playback"))]
        let managed_render_runtime = if self.managed_render_runtime {
            Some(ManagedRenderRuntime::start(
                &engine_config,
                &limits,
                playback_renderer
                    .take()
                    .expect("playback renderer is present during engine construction"),
            )?)
        } else {
            None
        };
        #[cfg(not(all(feature = "analysis", feature = "playback")))]
        if self.managed_render_runtime {
            return Err(AudioError::invalid_configuration(
                "managed rendering requires the analysis and playback features",
            ));
        }
        let backends = self.backends;
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("golden-audio-control".to_owned())
            .spawn(move || {
                let _ = worker_thread.set(thread::current());
                run_control_worker(
                    command_receiver,
                    ControlWorker {
                        event_sender,
                        observation,
                        state: worker_state,
                        engine_config,
                        limits,
                        backends,
                        pressure,
                        #[cfg(feature = "playback")]
                        playback_scheduler,
                        #[cfg(feature = "playback")]
                        playback_voice_controller,
                        #[cfg(all(feature = "analysis", feature = "playback"))]
                        managed_render_runtime,
                    },
                );
            })
            .map_err(|error| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    format!("failed to start audio control worker: {error}"),
                )
            })?;

        Ok(AudioEngine {
            control,
            raw_sender: command_sender,
            worker: Some(worker),
            events: Some(AudioEventReceiver {
                receiver: event_receiver,
            }),
            observations,
            #[cfg(feature = "playback")]
            playback_renderer,
        })
    }
}

#[derive(Debug)]
pub struct AudioEngine {
    control: AudioControl,
    raw_sender: CommandQueueProducer,
    worker: Option<JoinHandle<()>>,
    events: Option<AudioEventReceiver>,
    observations: AudioObservationReader,
    #[cfg(feature = "playback")]
    playback_renderer: Option<PlaybackVoiceRenderer>,
}

impl AudioEngine {
    #[must_use]
    pub fn control(&self) -> AudioControl {
        self.control.clone()
    }

    #[must_use]
    pub fn observations(&self) -> AudioObservationReader {
        self.observations.clone()
    }

    pub fn take_event_receiver(&mut self) -> Option<AudioEventReceiver> {
        self.events.take()
    }

    #[cfg(feature = "playback")]
    pub fn take_playback_renderer(&mut self) -> Option<PlaybackVoiceRenderer> {
        self.playback_renderer.take()
    }

    pub fn shutdown(&mut self) -> Result<(), AudioError> {
        if self.worker.is_none() {
            return Ok(());
        }
        let previous = self.control.state.swap(STATE_STOPPING, Ordering::AcqRel);
        if previous == STATE_RUNNING {
            let sequence = next_sequence(&self.control.next_sequence)?;
            self.raw_sender.push_blocking(CommandEnvelope {
                sequence,
                command: AudioCommand::Shutdown,
            })?;
        }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    "audio control worker panicked during shutdown",
                )
            })?;
        }
        #[cfg(feature = "playback")]
        if let Some(renderer) = self.playback_renderer.take() {
            renderer.into_retirement().reclaim();
        }
        self.control.state.store(STATE_STOPPED, Ordering::Release);
        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn next_sequence(counter: &AtomicU64) -> Result<CommandSequence, AudioError> {
    let value = counter.fetch_add(1, Ordering::Relaxed);
    CommandSequence::new(value).map_err(|error| {
        AudioError::new(
            AudioErrorCategory::InternalInvariant,
            format!("audio command sequence exhausted: {error}"),
        )
    })
}

struct ControlWorker {
    event_sender: SyncSender<AudioEvent>,
    observation: Arc<RwLock<AudioObservationSnapshot>>,
    state: Arc<AtomicU8>,
    engine_config: AudioEngineConfig,
    limits: EngineLimits,
    backends: Vec<Arc<dyn AudioBackend>>,
    pressure: QueuePressureCounters,
    #[cfg(feature = "playback")]
    playback_scheduler: PlaybackScheduler,
    #[cfg(feature = "playback")]
    playback_voice_controller: PlaybackVoiceController,
    #[cfg(all(feature = "analysis", feature = "playback"))]
    managed_render_runtime: Option<ManagedRenderRuntime>,
}

fn run_control_worker(mut command_receiver: Consumer<CommandEnvelope>, worker: ControlWorker) {
    let ControlWorker {
        event_sender,
        observation,
        state,
        engine_config,
        limits,
        backends,
        pressure,
        #[cfg(feature = "playback")]
        mut playback_scheduler,
        #[cfg(feature = "playback")]
        mut playback_voice_controller,
        #[cfg(all(feature = "analysis", feature = "playback"))]
        mut managed_render_runtime,
    } = worker;
    #[cfg(all(feature = "analysis", feature = "playback"))]
    if let Some(error) = managed_render_runtime
        .as_mut()
        .and_then(ManagedRenderRuntime::take_startup_priority_error)
    {
        publish_event(
            &event_sender,
            &observation,
            AudioEvent::Diagnostic(
                DiagnosticEvent::new(
                    DiagnosticSeverity::Warning,
                    "audio_render_realtime_priority_denied",
                    "audio rendering will continue without realtime thread priority",
                )
                .with_context("thread", "golden-audio-render")
                .with_context("block_frames", engine_config.internal_block_frames.get().to_string())
                .with_context("sample_rate", engine_config.sample_rate.get().to_string())
                .with_context("error", error),
            ),
        );
    }
    let mut devices = DeviceRuntime::new(&engine_config)
        .expect("validated audio engine configuration must create a device supervisor");
    devices.refresh(
        &event_sender,
        &observation,
        backends.as_slice(),
        true,
        #[cfg(all(feature = "analysis", feature = "playback"))]
        managed_render_runtime.as_mut(),
    );
    let mut reported_command_pressure = 0;
    #[cfg(feature = "playback")]
    let mut playback_lifecycle = PlaybackLifecycles::new();
    loop {
        #[cfg(all(feature = "analysis", feature = "playback"))]
        refresh_managed_render_observation(managed_render_runtime.as_mut(), &observation);
        #[cfg(feature = "playback")]
        drain_playback_work(
            &event_sender,
            &observation,
            &engine_config,
            &limits,
            &mut playback_scheduler,
            &mut playback_voice_controller,
            &mut playback_lifecycle,
        );
        let Ok(envelope) = command_receiver.pop() else {
            devices.refresh(
                &event_sender,
                &observation,
                backends.as_slice(),
                false,
                #[cfg(all(feature = "analysis", feature = "playback"))]
                managed_render_runtime.as_mut(),
            );
            report_command_pressure(
                &event_sender,
                &observation,
                &pressure,
                limits.command_queue_capacity,
                &mut reported_command_pressure,
            );
            thread::park_timeout(Duration::from_millis(if cfg!(feature = "playback") { 5 } else { 250 }));
            continue;
        };
        let sequence = envelope.sequence;
        match envelope.command {
            AudioCommand::ApplyConfiguration { generation, config } => {
                apply_configuration(
                    ApplyConfigurationContext {
                        event_sender: &event_sender,
                        observation: &observation,
                        engine_config: &engine_config,
                        limits: &limits,
                        backends: backends.as_slice(),
                        devices: &mut devices,
                        #[cfg(all(feature = "analysis", feature = "playback"))]
                        managed_render_runtime: managed_render_runtime.as_mut(),
                    },
                    generation,
                    *config,
                );
            }
            AudioCommand::SetEnabled(enabled) => {
                #[cfg(feature = "playback")]
                if !enabled {
                    let _ = stop_all_playback(
                        &event_sender,
                        &observation,
                        &mut playback_scheduler,
                        &mut playback_voice_controller,
                        &mut playback_lifecycle,
                        PlaybackStopReason::ModuleDisabled,
                    );
                }
                update_observation(&observation, |snapshot| {
                    snapshot.enabled = enabled;
                });
                devices.set_enabled(
                    &event_sender,
                    &observation,
                    backends.as_slice(),
                    enabled,
                    #[cfg(all(feature = "analysis", feature = "playback"))]
                    managed_render_runtime.as_mut(),
                );
            }
            AudioCommand::SetMasterGain { gain } => {
                let applied = devices.set_master_gain(gain);
                #[cfg(all(feature = "analysis", feature = "playback"))]
                let applied = applied.and_then(|()| {
                    managed_render_runtime
                        .as_mut()
                        .map_or(Ok(()), |runtime| runtime.set_master_gain(gain))
                });
                if let Err(error) = applied {
                    publish_command_failure(&event_sender, &observation, "set_master_gain", error);
                }
            }
            AudioCommand::SetChannelGain { channel, gain } => {
                let applied = devices.set_channel_gain(channel, gain);
                #[cfg(all(feature = "analysis", feature = "playback"))]
                let applied = applied.and_then(|()| {
                    managed_render_runtime
                        .as_mut()
                        .map_or(Ok(()), |runtime| runtime.set_channel_gain(channel, gain))
                });
                if let Err(error) = applied {
                    publish_command_failure(&event_sender, &observation, "set_channel_gain", error);
                }
            }
            AudioCommand::SetDeviceProbeInterests { targets } => {
                devices.set_probe_interests(
                    &event_sender,
                    &observation,
                    backends.as_slice(),
                    targets,
                    #[cfg(all(feature = "analysis", feature = "playback"))]
                    managed_render_runtime.as_mut(),
                );
            }
            AudioCommand::StopFile { playback_id } => {
                #[cfg(feature = "playback")]
                if !stop_playback(
                    &event_sender,
                    &observation,
                    &mut playback_scheduler,
                    &mut playback_voice_controller,
                    &mut playback_lifecycle,
                    &playback_id,
                    PlaybackStopReason::Requested,
                ) {
                    publish_event(
                        &event_sender,
                        &observation,
                        AudioEvent::PlaybackCommandIgnored(crate::PlaybackCommandIgnored::stop_file(
                            sequence,
                            playback_id,
                        )),
                    );
                }
                #[cfg(not(feature = "playback"))]
                publish_event(
                    &event_sender,
                    &observation,
                    AudioEvent::PlaybackCommandIgnored(crate::PlaybackCommandIgnored::stop_file(sequence, playback_id)),
                );
            }
            AudioCommand::StopAllFiles => {
                #[cfg(feature = "playback")]
                if !stop_all_playback(
                    &event_sender,
                    &observation,
                    &mut playback_scheduler,
                    &mut playback_voice_controller,
                    &mut playback_lifecycle,
                    PlaybackStopReason::StopAll,
                ) {
                    publish_event(
                        &event_sender,
                        &observation,
                        AudioEvent::PlaybackCommandIgnored(crate::PlaybackCommandIgnored::stop_all_files(sequence)),
                    );
                }
                #[cfg(not(feature = "playback"))]
                publish_event(
                    &event_sender,
                    &observation,
                    AudioEvent::PlaybackCommandIgnored(crate::PlaybackCommandIgnored::stop_all_files(sequence)),
                );
            }
            AudioCommand::PlayFile(request) => {
                #[cfg(feature = "playback")]
                schedule_playback(
                    &event_sender,
                    &observation,
                    &mut playback_scheduler,
                    &mut playback_voice_controller,
                    &mut playback_lifecycle,
                    sequence,
                    request,
                );
                #[cfg(not(feature = "playback"))]
                publish_event(
                    &event_sender,
                    &observation,
                    AudioEvent::PlaybackFailed(crate::PlaybackFailure {
                        playback_id: request.playback_id,
                        path: request.path,
                        error: AudioError::new(
                            AudioErrorCategory::DecodeFailed,
                            "file playback support is not compiled into this build",
                        ),
                    }),
                );
            }
            AudioCommand::Shutdown => break,
        }
        report_command_pressure(
            &event_sender,
            &observation,
            &pressure,
            limits.command_queue_capacity,
            &mut reported_command_pressure,
        );
    }
    devices.shutdown(
        &event_sender,
        &observation,
        #[cfg(all(feature = "analysis", feature = "playback"))]
        managed_render_runtime.as_mut(),
    );
    #[cfg(all(feature = "analysis", feature = "playback"))]
    if let Some(runtime) = &mut managed_render_runtime {
        let _ = runtime.shutdown();
    }
    publish_event(&event_sender, &observation, AudioEvent::ShutdownComplete);
    state.store(STATE_STOPPED, Ordering::Release);
}

#[cfg(all(feature = "analysis", feature = "playback"))]
fn refresh_managed_render_observation(
    runtime: Option<&mut ManagedRenderRuntime>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let (render, analysis) = runtime.refresh_observation();
    update_observation(observation, |snapshot| {
        snapshot.runtime = render;
        snapshot.render_frame = render.rendered_frames;
        if let Some(analysis) = analysis {
            snapshot.inputs = analysis.inputs.clone();
            snapshot.outputs = analysis.outputs.clone();
            snapshot.input_global_max_rms = analysis.input_global_max_rms;
            snapshot.output_global_max_rms = analysis.output_global_max_rms;
            snapshot.global_max_rms = analysis.global_max_rms;
            snapshot.analysis = analysis;
        }
    });
}

fn report_command_pressure(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    pressure: &QueuePressureCounters,
    capacity: usize,
    reported: &mut u64,
) {
    let occurrences = pressure.snapshot().command_full;
    if occurrences == *reported {
        return;
    }
    *reported = occurrences;
    update_observation(observation, |snapshot| {
        snapshot.queue_pressure_count = occurrences;
    });
    publish_event(
        event_sender,
        observation,
        AudioEvent::QueuePressure(QueuePressureEvent {
            queue: AudioQueueKind::Command,
            occurrences,
            capacity,
        }),
    );
}

fn publish_command_failure(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    operation: &str,
    error: AudioError,
) {
    publish_event(
        event_sender,
        observation,
        AudioEvent::Diagnostic(
            DiagnosticEvent::new(DiagnosticSeverity::Error, "audio_command_failed", error.to_string())
                .with_context("operation", operation)
                .with_context("category", format!("{:?}", error.category).to_lowercase()),
        ),
    );
}

pub(super) fn publish_event(
    sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    event: AudioEvent,
) {
    if sender.try_send(event).is_err() {
        update_observation(observation, |snapshot| {
            snapshot.dropped_event_count = snapshot.dropped_event_count.saturating_add(1);
        });
    }
}

pub(super) fn update_observation(
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    update: impl FnOnce(&mut AudioObservationSnapshot),
) {
    if let Ok(mut snapshot) = observation.write() {
        update(&mut snapshot);
    }
}
