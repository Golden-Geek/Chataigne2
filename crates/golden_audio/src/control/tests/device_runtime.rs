use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, mpsc::sync_channel},
};

use crate::{
    AudioBackend, AudioBackendState, AudioBufferPolicy, AudioConfiguration, AudioDeviceCatalogEntry,
    AudioDeviceDescriptor, AudioDeviceId, AudioDeviceInventory, AudioDeviceSelection, AudioDeviceTargetId,
    AudioDirection, AudioEngineConfig, AudioError, AudioErrorCategory, AudioObservationSnapshot, AudioRecoveryPolicy,
    AudioStream, BackendDescriptor, BackendId, DirectionConfiguration, EngineLimits, NullBackend, RenderCompileContext,
    RenderPlan, RenderPlanCompiler, StreamRequest,
};

use super::super::device_runtime::{DeviceRuntime, discover_backend_inventory, validate_stream_channel_inventory};

#[test]
fn stream_inventory_must_match_the_selected_device_count_and_order() {
    let devices = NullBackend.discover().unwrap();
    let device = devices.first().expect("null backend exposes one device");
    let full_output = device
        .output_channels
        .iter()
        .map(|channel| channel.key.clone())
        .collect::<Vec<_>>();

    validate_stream_channel_inventory(
        devices.as_slice(),
        &device.target,
        AudioDirection::Output,
        full_output.as_slice(),
    )
    .unwrap();

    assert!(
        validate_stream_channel_inventory(
            devices.as_slice(),
            &device.target,
            AudioDirection::Output,
            &full_output[1..],
        )
        .is_err()
    );

    let mut reordered = full_output;
    reordered.reverse();
    assert!(
        validate_stream_channel_inventory(
            devices.as_slice(),
            &device.target,
            AudioDirection::Output,
            reordered.as_slice(),
        )
        .is_err()
    );
}

#[derive(Clone)]
struct CatalogOnlyBackend {
    backend: BackendId,
    catalog: Vec<AudioDeviceCatalogEntry>,
    descriptors: HashMap<AudioDeviceTargetId, AudioDeviceDescriptor>,
    probes: Arc<Mutex<Vec<AudioDeviceTargetId>>>,
}

#[derive(Clone)]
struct FailingInventoryBackend {
    backend: BackendId,
}

#[derive(Clone, Default)]
struct TrackingNullBackend {
    opened: Arc<Mutex<Vec<AudioDirection>>>,
    stopped: Arc<Mutex<Vec<AudioDirection>>>,
}

struct TrackingStream {
    inner: Box<dyn AudioStream>,
    direction: AudioDirection,
    stopped: Arc<Mutex<Vec<AudioDirection>>>,
}

impl AudioStream for TrackingStream {
    fn status(&self) -> crate::AudioStreamStatus {
        self.inner.status()
    }

    fn start(&mut self) -> Result<(), AudioError> {
        self.inner.start()
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.stopped.lock().expect("stop log lock").push(self.direction);
        self.inner.stop()
    }
}

impl AudioBackend for TrackingNullBackend {
    fn id(&self) -> BackendId {
        NullBackend::backend_id()
    }

    fn descriptor(&self) -> BackendDescriptor {
        NullBackend.descriptor()
    }

    fn device_inventory(&self) -> Result<AudioDeviceInventory, AudioError> {
        NullBackend.device_inventory()
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        self.opened.lock().expect("open log lock").push(request.direction);
        Ok(Box::new(TrackingStream {
            inner: NullBackend.open_stream(request)?,
            direction: request.direction,
            stopped: Arc::clone(&self.stopped),
        }))
    }
}

impl AudioBackend for FailingInventoryBackend {
    fn id(&self) -> BackendId {
        self.backend.clone()
    }

    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: self.backend.clone(),
            label: "Failing Inventory".to_owned(),
            state: AudioBackendState::Available,
            detail: None,
        }
    }

    fn device_inventory(&self) -> Result<AudioDeviceInventory, AudioError> {
        Err(AudioError::new(
            AudioErrorCategory::BackendUnavailable,
            "transient inventory failure",
        ))
    }

    fn open_stream(&self, _request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        Err(AudioError::new(
            AudioErrorCategory::BackendUnavailable,
            "test backend does not open streams",
        ))
    }
}

impl AudioBackend for CatalogOnlyBackend {
    fn id(&self) -> BackendId {
        self.backend.clone()
    }

    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: self.backend.clone(),
            label: "Catalog Only".to_owned(),
            state: AudioBackendState::Available,
            detail: None,
        }
    }

    fn device_inventory(&self) -> Result<AudioDeviceInventory, AudioError> {
        Ok(AudioDeviceInventory {
            catalog: self.catalog.clone(),
            devices: Vec::new(),
        })
    }

    fn probe_device(&self, target: &AudioDeviceTargetId) -> Result<Option<AudioDeviceDescriptor>, AudioError> {
        self.probes.lock().expect("probe log lock").push(target.clone());
        Ok(self.descriptors.get(target).cloned())
    }

    fn open_stream(&self, _request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        Err(AudioError::new(
            AudioErrorCategory::BackendUnavailable,
            "catalog-only test backend does not open streams",
        ))
    }
}

#[test]
fn catalog_discovery_probes_only_explicit_interests_and_caches_the_result() {
    let backend = BackendId::from_static("catalog-only");
    let targets = ["first", "selected", "last"]
        .map(|name| AudioDeviceTargetId::Device {
            backend: backend.clone(),
            device: AudioDeviceId::new(name).unwrap(),
        })
        .to_vec();
    let template = NullBackend.discover().unwrap().into_iter().next().unwrap();
    let descriptors = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let mut descriptor = template.clone();
            descriptor.target = target.clone();
            descriptor.label = format!("Device {index}");
            (target.clone(), descriptor)
        })
        .collect::<HashMap<_, _>>();
    let catalog = targets
        .iter()
        .enumerate()
        .map(|(index, target)| AudioDeviceCatalogEntry {
            target: target.clone(),
            label: format!("Device {index}"),
        })
        .collect();
    let probes = Arc::new(Mutex::new(Vec::new()));
    let backends: Vec<Arc<dyn AudioBackend>> = vec![Arc::new(CatalogOnlyBackend {
        backend,
        catalog,
        descriptors,
        probes: Arc::clone(&probes),
    })];
    let (event_sender, _events) = sync_channel(32);
    let observation = Arc::new(RwLock::new(AudioObservationSnapshot::default()));
    let mut cache = HashMap::new();
    let mut failures = HashMap::new();

    let (_, catalog, devices) = discover_backend_inventory(
        &event_sender,
        &observation,
        backends.as_slice(),
        &targets[1..2],
        &mut cache,
        &mut failures,
    );
    assert_eq!(catalog.len(), 3);
    assert_eq!(
        devices.iter().map(|device| &device.target).collect::<Vec<_>>(),
        vec![&targets[1]]
    );
    assert_eq!(probes.lock().unwrap().as_slice(), &targets[1..2]);

    let (_, _, devices) = discover_backend_inventory(
        &event_sender,
        &observation,
        backends.as_slice(),
        &targets[1..2],
        &mut cache,
        &mut failures,
    );
    assert_eq!(devices.len(), 1);
    assert_eq!(probes.lock().unwrap().as_slice(), &targets[1..2]);

    let (_, _, devices) = discover_backend_inventory(
        &event_sender,
        &observation,
        backends.as_slice(),
        &targets[2..3],
        &mut cache,
        &mut failures,
    );
    assert_eq!(devices[0].target, targets[2]);
    assert_eq!(
        probes.lock().unwrap().as_slice(),
        &[targets[1].clone(), targets[2].clone()]
    );
}

#[test]
fn transient_backend_failure_keeps_the_last_known_device_catalog() {
    let backend = BackendId::from_static("flaky");
    let target = AudioDeviceTargetId::Device {
        backend: backend.clone(),
        device: AudioDeviceId::from_static("headphones"),
    };
    let catalog_entry = AudioDeviceCatalogEntry {
        target,
        label: "Headphones".to_owned(),
    };
    let backends: Vec<Arc<dyn AudioBackend>> = vec![Arc::new(FailingInventoryBackend {
        backend: backend.clone(),
    })];
    let (event_sender, _events) = sync_channel(32);
    let mut previous = AudioObservationSnapshot::default();
    previous.device.device_catalog = vec![catalog_entry.clone()];
    let observation = Arc::new(RwLock::new(previous));

    let (statuses, catalog, devices) = discover_backend_inventory(
        &event_sender,
        &observation,
        backends.as_slice(),
        &[],
        &mut HashMap::new(),
        &mut HashMap::new(),
    );

    assert_eq!(statuses[0].state, AudioBackendState::Failed);
    assert_eq!(catalog, vec![catalog_entry]);
    assert!(
        devices.is_empty(),
        "stale capabilities must not be treated as a live device",
    );
}

#[test]
fn changing_from_output_only_to_input_only_stops_output_without_reopening_it() {
    let backend = TrackingNullBackend::default();
    let opened = Arc::clone(&backend.opened);
    let stopped = Arc::clone(&backend.stopped);
    let backends: Vec<Arc<dyn AudioBackend>> = vec![Arc::new(backend)];
    let device = NullBackend
        .discover()
        .expect("null inventory")
        .into_iter()
        .next()
        .expect("null device");
    let engine_config = AudioEngineConfig::default();
    let limits = EngineLimits::default();
    let (event_sender, _events) = sync_channel(64);
    let observation = Arc::new(RwLock::new(AudioObservationSnapshot::default()));
    let mut runtime = DeviceRuntime::new(&engine_config).expect("device runtime");

    let output = single_direction_configuration(&device, AudioDirection::Output);
    runtime.configure(
        &event_sender,
        &observation,
        backends.as_slice(),
        &output,
        compile_plan(&engine_config, &limits, &output),
        #[cfg(all(feature = "analysis", feature = "playback"))]
        None,
    );
    assert_eq!(
        opened.lock().expect("open log lock").as_slice(),
        &[AudioDirection::Output],
    );

    let input = single_direction_configuration(&device, AudioDirection::Input);
    runtime.configure(
        &event_sender,
        &observation,
        backends.as_slice(),
        &input,
        compile_plan(&engine_config, &limits, &input),
        #[cfg(all(feature = "analysis", feature = "playback"))]
        None,
    );

    assert_eq!(
        stopped.lock().expect("stop log lock").as_slice(),
        &[AudioDirection::Output],
        "disabling output must stop its active stream before input is opened",
    );
    assert_eq!(
        opened.lock().expect("open log lock").as_slice(),
        &[AudioDirection::Output, AudioDirection::Input],
        "an input-only configuration must not reopen output",
    );
    let state = observation.read().expect("observation lock");
    assert!(!state.device.output.enabled);
    assert!(state.device.input.enabled);
}

fn single_direction_configuration(device: &AudioDeviceDescriptor, direction: AudioDirection) -> AudioConfiguration {
    let mut configuration = AudioConfiguration::empty();
    let direction_configuration = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::from_descriptor(device)),
        recovery_policy: AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: AudioBufferPolicy::Automatic,
    };
    match direction {
        AudioDirection::Input => {
            configuration.input = direction_configuration;
            configuration.physical_inputs = device
                .input_channels
                .iter()
                .map(|channel| channel.key.clone())
                .collect();
        }
        AudioDirection::Output => {
            configuration.output = direction_configuration;
            configuration.physical_outputs = device
                .output_channels
                .iter()
                .map(|channel| channel.key.clone())
                .collect();
        }
    }
    configuration
}

fn compile_plan(engine: &AudioEngineConfig, limits: &EngineLimits, configuration: &AudioConfiguration) -> RenderPlan {
    RenderPlanCompiler::new(engine.clone(), limits.clone())
        .compile(
            configuration,
            &RenderCompileContext::derive_from_configuration(configuration),
        )
        .expect("valid test render plan")
        .plan
}
