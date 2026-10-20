use crate::{AudioChannelId, AudioConfiguration, AudioDeviceSelection, AudioDirection, BackendId, VirtualInputChannel};

#[test]
fn configuration_storage_bounds_spare_vectors_labels_and_nested_device_metadata() {
    let mut config = AudioConfiguration::empty();
    assert!(config.retained_bytes_within(4096, 1).is_some());
    config.virtual_inputs = Vec::with_capacity(4096);
    assert!(config.retained_bytes_within(4096, 1).is_none());
    config.virtual_inputs = vec![VirtualInputChannel {
        id: AudioChannelId::new(),
        label: String::with_capacity(8192),
    }];
    assert!(config.retained_bytes_within(4096, 2).is_none());
    config.virtual_inputs.clear();
    let mut selection =
        AudioDeviceSelection::follow_system_default(BackendId::from_static("null"), AudioDirection::Input);
    selection.last_known_label = String::with_capacity(8192);
    config.input.device = Some(selection);
    assert!(config.retained_bytes_within(4096, 100).is_none());
    let bytes = config.retained_bytes_within(1024 * 1024, 100).unwrap();
    assert!(config.retained_bytes_within(bytes - 1, 100).is_none());
}
