//! Owns Golden Audio and reconciles its device inventory on a cancellable host worker.
use super::{
    configuration,
    preferences::{Publication, Settings, Status},
};
use crate::app::AppNode;
use golden_audio::{
    AudioCommand, AudioConfiguration, AudioDeviceReadiness, AudioEngine, AudioEngineBuilder, AudioEvent, BackendId,
    BackendPolicy, ConfigGeneration, compiled_cpal_backend_catalog, compiled_cpal_backends,
};
use golden_core::application::ProductionRuntime;
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits};
use std::{
    io,
    num::NonZeroUsize,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

pub(crate) struct Service {
    _worker: BoundedWorker<(), ()>,
}

pub(crate) fn start(runtime: ProductionRuntime<AppNode>) -> io::Result<Service> {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    let pool = POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(2).unwrap()));
    let worker = BoundedWorker::spawn(
        "mapgyver-audio",
        pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |context| {
            let (engine, backend) = native_engine();
            let mut engine = engine.map(AudioOwner::new);
            while !context.cancellation().is_cancelled() {
                let checkpoint = context.wake_checkpoint();
                if let Ok(Ok(settings)) = runtime.capture_app(Settings::capture) {
                    let update = match &mut engine {
                        Ok(owner) => owner.reconcile(&settings, backend.as_ref()),
                        Err(error) => settings_failure(&settings, error.clone()),
                    };
                    if update.status != settings.published || !update.choices.is_empty() {
                        settings.publish(update);
                    }
                }
                context.wait_until(checkpoint, Instant::now() + Duration::from_millis(400));
            }
        },
    )
    .map_err(io::Error::other)?;
    Ok(Service { _worker: worker })
}

fn native_engine() -> (Result<AudioEngine, String>, Option<BackendId>) {
    let backend = compiled_cpal_backend_catalog()
        .into_iter()
        .find(|info| info.is_platform_default && info.supports_system_default)
        .map(|info| info.id);
    let mut builder = AudioEngineBuilder::default()
        .without_backends()
        .with_managed_render_runtime();
    for device_backend in compiled_cpal_backends() {
        builder = builder.with_boxed_backend(device_backend);
    }
    if let Some(backend) = &backend {
        builder.backend_policy = BackendPolicy {
            preferred: vec![backend.clone()],
            allow_null_fallback: false,
        };
    }
    (builder.build().map_err(|error| error.to_string()), backend)
}

struct AudioOwner {
    engine: AudioEngine,
    events: golden_audio::AudioEventReceiver,
    submitted: Option<Arc<AudioConfiguration>>,
    generation: ConfigGeneration,
    error: Option<String>,
}
impl AudioOwner {
    fn new(mut engine: AudioEngine) -> Self {
        let events = engine.take_event_receiver().expect("new Golden Audio event receiver");
        Self {
            engine,
            events,
            submitted: None,
            generation: ConfigGeneration::INITIAL,
            error: None,
        }
    }

    fn reconcile(&mut self, settings: &Settings, backend: Option<&BackendId>) -> Publication {
        for event in std::iter::from_fn(|| self.events.try_recv()).take(512) {
            if let AudioEvent::ConfigurationRejected { generation, error } = event
                && generation == self.generation
            {
                self.error = Some(error.to_string());
            }
        }
        let (inventory, generation) = self
            .engine
            .observations()
            .inspect(|state| (state.device.clone(), state.generation));
        let (config, choices) = match configuration::resolve(settings, backend, &inventory) {
            Ok(config) => config,
            Err(error) => {
                self.submit(AudioConfiguration::empty());
                return settings_failure(settings, error);
            }
        };
        let waiting_for_device = settings.enabled && !config.output.enabled;
        self.submit(config);
        let output = &inventory.output;
        let missing =
            waiting_for_device && generation == self.generation && output.readiness == AudioDeviceReadiness::Disabled;
        let mut physical_channels = Vec::new();
        let mut routes = Vec::new();
        if generation == self.generation
            && let Some(config) = &self.submitted
        {
            physical_channels = config.physical_outputs.iter().map(ToString::to_string).collect();
            routes = config
                .output_patch
                .iter()
                .map(|route| (route.source.to_string(), route.destination.to_string()))
                .collect();
        }
        Publication {
            status: Status {
                enabled: settings.enabled,
                selected: settings.device.value.clone(),
                readiness: Some(if missing {
                    AudioDeviceReadiness::Missing
                } else {
                    output.readiness
                }),
                active_device: output.active_target.clone(),
                format: output.format.clone(),
                physical_channels,
                routes,
                configuration_generation: generation.get(),
                error: self
                    .error
                    .clone()
                    .or_else(|| output.error.as_ref().map(|error| error.message.clone()))
                    .or_else(|| missing.then(|| "Selected audio output is unavailable".into())),
            },
            choices,
        }
    }

    fn submit(&mut self, config: AudioConfiguration) {
        if self.submitted.as_deref() == Some(&config) {
            return;
        }
        let next = self.generation.next();
        let config = Arc::new(config);
        match self.engine.control().submit(AudioCommand::ApplyConfiguration {
            generation: next,
            config: config.clone(),
        }) {
            Ok(_) => {
                self.generation = next;
                self.submitted = Some(config);
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

fn settings_failure(settings: &Settings, error: String) -> Publication {
    Publication {
        status: Status {
            enabled: settings.enabled,
            selected: settings.device.value.clone(),
            error: Some(error),
            ..Status::default()
        },
        choices: vec![],
    }
}
