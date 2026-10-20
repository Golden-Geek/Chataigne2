use super::*;

#[test]
fn device_revision_reads_do_not_wait_for_snapshot_publication() {
    use std::{sync::mpsc, time::Duration};
    let reader = AudioObservationReader {
        shared: Arc::new(RwLock::new(AudioObservationSnapshot::default())),
        device_revision: Arc::new(AtomicU64::new(7)),
    };
    let writer = reader.shared.write().unwrap();
    let (sender, receiver) = mpsc::channel();
    let worker_reader = reader.clone();
    let worker = std::thread::spawn(move || sender.send(worker_reader.device_revision()).unwrap());
    let result = receiver.recv_timeout(Duration::from_secs(1));
    drop(writer);
    worker.join().unwrap();
    assert_eq!(result.unwrap(), 7);
}

#[test]
fn native_device_revision_is_not_a_wire_or_saved_observation_field() {
    let snapshot = AudioObservationSnapshot {
        device_revision: 7,
        ..Default::default()
    };
    let json = serde_json::to_value(&snapshot).unwrap();
    assert!(json.get("device_revision").is_none());
    assert_eq!(
        serde_json::from_value::<AudioObservationSnapshot>(json)
            .unwrap()
            .device_revision,
        0
    );
}

#[test]
fn inspection_borrows_canonical_storage_before_any_snapshot_copy() {
    let snapshot = AudioObservationSnapshot {
        inputs: Vec::with_capacity(4096),
        ..Default::default()
    };
    let original = snapshot.inputs.as_ptr();
    let reader = AudioObservationReader {
        device_revision: Arc::new(AtomicU64::new(0)),
        shared: Arc::new(RwLock::new(snapshot)),
    };
    reader.inspect(|snapshot| {
        assert_eq!(snapshot.inputs.as_ptr(), original);
        assert!(snapshot.retained_bytes_within(1024, 100).is_none());
    });
    assert!(reader.latest().inputs.capacity() < 4096);
}

#[test]
fn observation_weight_bounds_collection_walks_and_owned_spare_storage() {
    let mut snapshot = AudioObservationSnapshot::default();
    assert!(snapshot.retained_bytes_within(4096, 1).is_some());
    snapshot.device.input.selected_label = Some(String::with_capacity(8192));
    assert!(snapshot.retained_bytes_within(4096, 100).is_none());
    snapshot.device.input.selected_label = None;
    snapshot.device.device_catalog = (0..10)
        .map(|index| crate::AudioDeviceCatalogEntry {
            target: crate::AudioDeviceTargetId::SystemDefault {
                backend: crate::BackendId::from_static("null"),
            },
            label: index.to_string(),
        })
        .collect();
    assert!(snapshot.retained_bytes_within(1024 * 1024, 10).is_none());
    let bytes = snapshot.retained_bytes_within(1024 * 1024, 11).unwrap();
    assert!(snapshot.retained_bytes_within(bytes - 1, 11).is_none());
}

#[test]
fn nested_analysis_storage_and_entries_are_bounded_before_copying() {
    let mut snapshot = AudioObservationSnapshot::default();
    let mut pitch = crate::PitchObservation::invalid();
    pitch.note_name = String::with_capacity(8192);
    snapshot.analysis.taps.push(crate::AnalysisTapObservation {
        tap: crate::AnalysisTapId::from_uuid(uuid::Uuid::from_u128(1)),
        source: crate::AudioChannelId::from_uuid(uuid::Uuid::from_u128(2)),
        enabled: true,
        result: Some(crate::AnalysisResult::Pitch(pitch)),
    });
    assert!(snapshot.retained_bytes_within(4096, 100).is_none());
    snapshot.analysis.taps[0].result = Some(crate::AnalysisResult::Spectrum(crate::SpectrumObservation {
        fft_size: 1024,
        bands: Vec::with_capacity(1024),
    }));
    assert!(snapshot.retained_bytes_within(4096, 100).is_none());
    let Some(crate::AnalysisResult::Spectrum(spectrum)) = &mut snapshot.analysis.taps[0].result else {
        unreachable!()
    };
    spectrum.bands = vec![
        crate::SpectrumBandObservation {
            index: 0,
            low_hz: 20.0,
            center_hz: 40.0,
            high_hz: 80.0,
            amplitude_linear: 0.0,
            amplitude_dbfs: -120.0,
        };
        10
    ];
    assert!(snapshot.retained_bytes_within(1024 * 1024, 11).is_none());
    assert!(snapshot.retained_bytes_within(1024 * 1024, 12).is_some());
}

#[test]
fn device_fingerprints_and_physical_channels_charge_nested_capacity() {
    let mut snapshot = AudioObservationSnapshot::default();
    let target = crate::AudioDeviceTargetId::SystemDefault {
        backend: crate::BackendId::from_static("null"),
    };
    snapshot.device.devices.push(crate::AudioDeviceDescriptor {
        profile_key: crate::profile_key_for(&target, None, None),
        target,
        label: "fixture".into(),
        stable_id: true,
        fingerprint: crate::AudioDeviceFingerprint::default(),
        input_channels: vec![crate::PhysicalChannelDescriptor {
            key: crate::PhysicalChannelKey::new("input:0").unwrap(),
            label: String::with_capacity(8192),
            position: None,
        }],
        output_channels: vec![],
        supported_configurations: vec![],
        is_system_default_input: false,
        is_system_default_output: false,
    });
    assert!(snapshot.retained_bytes_within(4096, 100).is_none());
    snapshot.device.devices[0].input_channels.clear();
    snapshot.device.devices[0]
        .fingerprint
        .properties
        .insert("opaque".into(), String::with_capacity(8192));
    assert!(snapshot.retained_bytes_within(4096, 100).is_none());
    assert!(snapshot.retained_bytes_within(1024 * 1024, 2).is_none());
    assert!(snapshot.retained_bytes_within(1024 * 1024, 3).is_some());
}

#[test]
fn device_metadata_admission_is_independent_of_large_metering_storage() {
    let snapshot = AudioObservationSnapshot {
        inputs: Vec::with_capacity(100_000),
        ..Default::default()
    };
    assert!(snapshot.retained_bytes_within(4096, 100).is_none());
    assert!(snapshot.device.retained_bytes_within(4096, 100).is_some());
}
