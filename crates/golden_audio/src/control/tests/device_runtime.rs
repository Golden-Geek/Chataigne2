use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, mpsc::sync_channel},
};

use crate::{
    AudioBackend, AudioBackendState, AudioDeviceCatalogEntry, AudioDeviceDescriptor, AudioDeviceId,
    AudioDeviceInventory, AudioDeviceTargetId, AudioDirection, AudioError, AudioErrorCategory,
    AudioObservationSnapshot, AudioStream, BackendDescriptor, BackendId, NullBackend, StreamRequest,
};

use super::super::device_runtime::{discover_backend_inventory, validate_stream_channel_inventory};

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
