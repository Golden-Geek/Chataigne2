use super::*;
use golden_audio::{
    AudioBackend, AudioDeviceCatalogEntry, AudioDeviceId, AudioDeviceInspectorState, AudioDeviceSelection,
    AudioDeviceTargetId, NullBackend,
};

#[test]
fn repeated_catalog_targets_keep_one_selectable_identity() {
    let devices = NullBackend.discover().unwrap();
    let entry = AudioDeviceCatalogEntry::from(&devices[0]);
    let state = AudioDeviceInspectorState {
        devices,
        device_catalog: vec![entry.clone(), entry],
        ..Default::default()
    };
    let options = device_options_for_current(
        NO_AUDIO_DEVICE,
        &state,
        DeviceChoice::Input,
        Some(&NullBackend::backend_id()),
    )
    .unwrap();
    assert_eq!(options.len(), 3, "None, System Default and one catalog target");
    assert_eq!(
        options
            .iter()
            .map(|option| option.variant_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
}

#[test]
fn large_catalog_has_explicit_item_admission_instead_of_unbounded_option_growth() {
    let device_catalog = (0..MAX_OPTIONS)
        .map(|index| AudioDeviceCatalogEntry {
            target: AudioDeviceTargetId::Device {
                backend: NullBackend::backend_id(),
                device: AudioDeviceId::new(format!("device-{index}")).unwrap(),
            },
            label: format!("Device {index}"),
        })
        .collect();
    let state = AudioDeviceInspectorState {
        device_catalog,
        ..Default::default()
    };
    let error = device_options_for_current(
        NO_AUDIO_DEVICE,
        &state,
        DeviceChoice::Input,
        Some(&NullBackend::backend_id()),
    )
    .unwrap_err();
    assert!(error.contains("32768"));
}

#[test]
fn serialized_identity_admission_counts_escaped_json_bytes() {
    let device = NullBackend.discover().unwrap().remove(0);
    let mut selection = AudioDeviceSelection::from_descriptor(&device);
    selection.last_known_label = "\0".repeat(MAX_ID_BYTES / 6 + 1);
    assert!(selection.last_known_label.len() < MAX_ID_BYTES);
    assert!(selection_value(&selection).unwrap_err().contains("1 MiB"));
    selection.last_known_label = "Readable device".into();
    assert_eq!(
        selection_value(&selection).unwrap(),
        serde_json::to_string(&selection).unwrap()
    );
}
