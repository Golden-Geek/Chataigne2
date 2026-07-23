use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicU8, AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use rtrb::Consumer;

use crate::{
    AudioBackend, AudioBackendState, AudioBackendStatus, AudioCommand, AudioConfiguration, AudioDeviceInspectorState,
    AudioEngineConfig, AudioError, AudioErrorCategory, AudioEvent, AudioObservationReader, AudioObservationSnapshot,
    AudioQueueKind, BackendPolicy, CommandSequence, ConfigGeneration, EngineLimits, NullBackend, QueuePressureCounters,
    QueuePressureEvent, RenderCompileContext, RenderPlanCompiler,
};

use super::ingress::{CommandEnvelope, CommandQueueProducer, command_queue};

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
}

#[derive(Debug)]
pub struct AudioEngineBuilder {
    pub config: crate::AudioEngineConfig,
    pub limits: EngineLimits,
    pub backend_policy: BackendPolicy,
    backends: Vec<Arc<dyn AudioBackend>>,
}

impl Default for AudioEngineBuilder {
    fn default() -> Self {
        Self {
            config: crate::AudioEngineConfig::default(),
            limits: EngineLimits::default(),
            backend_policy: BackendPolicy::default(),
            backends: vec![Arc::new(NullBackend)],
        }
    }
}

impl AudioEngineBuilder {
    #[must_use]
    pub fn with_backend(mut self, backend: impl AudioBackend + 'static) -> Self {
        let id = backend.descriptor().id;
        self.backends.retain(|current| current.descriptor().id != id);
        self.backends.push(Arc::new(backend));
        self
    }

    #[must_use]
    pub fn without_backends(mut self) -> Self {
        self.backends.clear();
        self
    }

    pub fn build(self) -> Result<AudioEngine, AudioError> {
        self.config.validate()?;
        self.limits.validate()?;
        validate_backends(&self.backends, &self.backend_policy)?;

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

fn validate_backends(backends: &[Arc<dyn AudioBackend>], policy: &BackendPolicy) -> Result<(), AudioError> {
    let mut ids = std::collections::HashSet::with_capacity(backends.len());
    for backend in backends {
        let id = backend.descriptor().id;
        if !ids.insert(id.clone()) {
            return Err(AudioError::invalid_configuration(format!(
                "duplicate audio backend ID {id}"
            )));
        }
    }
    for preferred in &policy.preferred {
        if !ids.contains(preferred) && !(policy.allow_null_fallback && preferred == &NullBackend::backend_id()) {
            return Err(AudioError::invalid_configuration(format!(
                "preferred audio backend {preferred} was not registered"
            )));
        }
    }
    if backends.is_empty() && !policy.allow_null_fallback {
        return Err(AudioError::invalid_configuration(
            "audio engine has no registered backend and null fallback is disabled",
        ));
    }
    Ok(())
}

struct ControlWorker {
    event_sender: SyncSender<AudioEvent>,
    observation: Arc<RwLock<AudioObservationSnapshot>>,
    state: Arc<AtomicU8>,
    engine_config: AudioEngineConfig,
    limits: EngineLimits,
    backends: Vec<Arc<dyn AudioBackend>>,
    pressure: QueuePressureCounters,
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
    } = worker;
    publish_backend_inventory(&event_sender, &observation, backends.as_slice());
    let mut reported_command_pressure = 0;
    loop {
        let Ok(envelope) = command_receiver.pop() else {
            report_command_pressure(
                &event_sender,
                &observation,
                &pressure,
                limits.command_queue_capacity,
                &mut reported_command_pressure,
            );
            thread::park_timeout(Duration::from_millis(250));
            continue;
        };
        let _sequence = envelope.sequence;
        match envelope.command {
            AudioCommand::ApplyConfiguration { generation, config } => {
                apply_configuration(
                    &event_sender,
                    &observation,
                    &engine_config,
                    &limits,
                    generation,
                    *config,
                );
            }
            AudioCommand::SetEnabled(enabled) => update_observation(&observation, |snapshot| {
                snapshot.enabled = enabled;
            }),
            AudioCommand::SetMasterGain { .. }
            | AudioCommand::SetChannelGain { .. }
            | AudioCommand::StopFile { .. }
            | AudioCommand::StopAllFiles => {}
            AudioCommand::PlayFile(request) => {
                let event = AudioEvent::PlaybackFailed(crate::PlaybackFailure {
                    playback_id: request.playback_id,
                    path: request.path,
                    error: AudioError::new(
                        AudioErrorCategory::DecodeFailed,
                        "file playback is not enabled in the backend-independent foundation",
                    ),
                });
                publish_event(&event_sender, &observation, event);
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
    publish_event(&event_sender, &observation, AudioEvent::ShutdownComplete);
    state.store(STATE_STOPPED, Ordering::Release);
}

fn apply_configuration(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    engine_config: &AudioEngineConfig,
    limits: &EngineLimits,
    generation: ConfigGeneration,
    config: AudioConfiguration,
) {
    let active_generation = observation
        .read()
        .map(|snapshot| snapshot.generation)
        .unwrap_or(ConfigGeneration::INITIAL);
    if generation <= active_generation {
        publish_event(
            event_sender,
            observation,
            AudioEvent::ConfigurationRejected {
                generation,
                error: AudioError::invalid_configuration(format!(
                    "configuration generation {generation} is not newer than active generation {active_generation}"
                )),
            },
        );
        return;
    }
    let context = RenderCompileContext::derive_from_configuration(&config);
    match RenderPlanCompiler::new(engine_config.clone(), limits.clone()).compile(&config, &context) {
        Ok(_compilation) => {
            update_observation(observation, |snapshot| {
                snapshot.generation = generation;
                snapshot.enabled = config.enabled;
            });
            publish_event(
                event_sender,
                observation,
                AudioEvent::ConfigurationApplied { generation },
            );
        }
        Err(error) => publish_event(
            event_sender,
            observation,
            AudioEvent::ConfigurationRejected { generation, error },
        ),
    }
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

fn publish_backend_inventory(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    backends: &[Arc<dyn AudioBackend>],
) {
    let mut statuses = Vec::with_capacity(backends.len());
    let mut devices = Vec::new();
    for backend in backends {
        let descriptor = backend.descriptor();
        let mut status = AudioBackendStatus {
            backend: descriptor.id,
            label: descriptor.label,
            state: descriptor.state,
            detail: descriptor.detail,
        };
        if status.state == AudioBackendState::Available {
            match backend.discover() {
                Ok(discovered) => devices.extend(discovered),
                Err(error) => {
                    status.state = AudioBackendState::Failed;
                    status.detail = Some(error.to_string());
                }
            }
        }
        publish_event(
            event_sender,
            observation,
            AudioEvent::BackendStatusChanged(status.clone()),
        );
        statuses.push(status);
    }
    update_observation(observation, |snapshot| {
        snapshot.device.backends = statuses;
        snapshot.device.devices = devices;
        snapshot.device.discovery_in_progress = false;
    });
}

fn publish_event(
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

fn update_observation(
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    update: impl FnOnce(&mut AudioObservationSnapshot),
) {
    if let Ok(mut snapshot) = observation.write() {
        update(&mut snapshot);
    }
}
