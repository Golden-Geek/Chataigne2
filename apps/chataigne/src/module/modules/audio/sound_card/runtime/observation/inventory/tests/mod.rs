use super::*;
use golden_audio::{
    AudioBackend, AudioDeviceId, AudioObservationSnapshot, BackendId, NullBackend, PhysicalChannelDescriptor,
    PhysicalChannelKey,
};

fn snapshot() -> AudioObservationSnapshot {
    let device = NullBackend.discover().unwrap().remove(0);
    let mut snapshot = AudioObservationSnapshot {
        device_revision: 7,
        ..Default::default()
    };
    snapshot.device.input.selected_target = Some(device.target.clone());
    snapshot.device.output.selected_target = Some(device.target.clone());
    snapshot.device.devices.push(device);
    snapshot
}

fn prepared(snapshot: &AudioObservationSnapshot) -> DeviceView {
    let pool = PayloadPool::new("route-frame-test", 1, FRAME_BYTES).unwrap();
    let mut frame = InventoryPreparer::draft(snapshot).unwrap();
    frame.sort();
    let bytes = frame.retained_bytes().unwrap();
    DeviceView {
        revision: frame.revision,
        frame: Some(pool.try_retain(frame, bytes).unwrap()),
        error: None,
    }
}

pub(crate) fn fixture_inventory(device: AudioDeviceInspectorState, revision: u64) -> DeviceInventory {
    prepared(&AudioObservationSnapshot {
        device,
        device_revision: revision,
        ..Default::default()
    })
    .catalog(revision)
    .unwrap()
}

#[test]
fn captured_inventory_routes_use_direction_and_borrowed_keys() {
    let snapshot = snapshot();
    let index = prepared(&snapshot);
    for direction in [AudioDirection::Input, AudioDirection::Output] {
        let channels = match direction {
            AudioDirection::Input => &snapshot.device.devices[0].input_channels,
            AudioDirection::Output => &snapshot.device.devices[0].output_channels,
        };
        for channel in channels {
            assert_eq!(index.validate(7, direction, channel.key.as_str()), Ok(()));
        }
        assert_eq!(
            index.validate(7, direction, "absent"),
            Err(RouteValidationError::UnavailableChannel)
        );
    }
    let key = snapshot.device.devices[0].input_channels[0].key.as_str();
    assert_eq!(
        index.validate(7, AudioDirection::Output, key),
        Err(RouteValidationError::UnavailableChannel)
    );
}

#[test]
fn route_index_fences_stale_metadata_before_accepting_a_matching_key() {
    let snapshot = snapshot();
    let index = prepared(&snapshot);
    let key = snapshot.device.devices[0].output_channels[0].key.as_str();
    assert_eq!(
        index.validate(8, AudioDirection::Output, key),
        Err(RouteValidationError::Stale)
    );
}

#[test]
fn active_endpoint_takes_precedence_over_a_pending_selection() {
    let mut snapshot = snapshot();
    let target = snapshot.device.devices[0].target.clone();
    snapshot.device.output.active_target = Some(target);
    snapshot.device.output.selected_target = Some(AudioDeviceTargetId::Device {
        backend: NullBackend::backend_id(),
        device: AudioDeviceId::new("not-yet-active").unwrap(),
    });
    let index = prepared(&snapshot);
    let key = snapshot.device.devices[0].output_channels[0].key.clone();
    assert_eq!(index.validate(7, AudioDirection::Output, key.as_str()), Ok(()));
    snapshot.device.output.active_target = None;
    let index = prepared(&snapshot);
    assert_eq!(
        index.validate(7, AudioDirection::Output, key.as_str()),
        Err(RouteValidationError::NoDevice)
    );
}

#[test]
fn system_default_resolution_respects_backend_and_direction() {
    let mut snapshot = snapshot();
    let mut other = snapshot.device.devices[0].clone();
    other.target = AudioDeviceTargetId::Device {
        backend: BackendId::from_static("other"),
        device: AudioDeviceId::new("default").unwrap(),
    };
    other.output_channels[0].key = PhysicalChannelKey::new("wrong-backend").unwrap();
    snapshot.device.devices.insert(0, other);
    snapshot.device.output.selected_target = Some(AudioDeviceTargetId::SystemDefault {
        backend: NullBackend::backend_id(),
    });
    let index = prepared(&snapshot);
    let key = snapshot.device.devices[1].output_channels[0].key.clone();
    assert_eq!(index.validate(7, AudioDirection::Output, key.as_str()), Ok(()));
    assert_eq!(
        index.validate(7, AudioDirection::Output, "wrong-backend"),
        Err(RouteValidationError::UnavailableChannel)
    );
    snapshot.device.devices[1].is_system_default_output = false;
    let index = prepared(&snapshot);
    assert_eq!(
        index.validate(7, AudioDirection::Output, key.as_str()),
        Err(RouteValidationError::NoDevice)
    );
}

#[test]
fn large_reversed_channel_inventory_is_sorted_natively_for_borrowed_lookup() {
    let mut snapshot = snapshot();
    snapshot.device.devices[0].output_channels = (0..32_768)
        .rev()
        .map(|index| PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new(format!("output:{index:05}")).unwrap(),
            label: format!("Channel {index}"),
            position: None,
        })
        .collect();
    let index = prepared(&snapshot);
    let output = index.frame.as_ref().unwrap().output.as_ref().unwrap();
    assert_eq!(output.channels.len(), 32_768);
    assert!(index.frame.as_ref().unwrap().retained_bytes().unwrap() < 4 * 1024 * 1024);
    for key in ["output:00000", "output:16384", "output:32767"] {
        assert_eq!(index.validate(7, AudioDirection::Output, key), Ok(()));
    }
    assert_eq!(
        index.validate(7, AudioDirection::Output, "output:32768"),
        Err(RouteValidationError::UnavailableChannel)
    );
}

#[test]
fn oversized_metering_does_not_block_route_metadata_preparation() {
    let mut snapshot = snapshot();
    snapshot.inputs = vec![
        golden_audio::ChannelObservation {
            channel: golden_audio::AudioChannelId::new(),
            rms_linear: 0.0,
            rms_dbfs: -120.0,
            peak_dbfs: -120.0,
            clipped: false,
        };
        65_537
    ];
    assert!(snapshot.retained_bytes_within(FRAME_BYTES, 65_536).is_none());
    let routes = prepared(&snapshot);
    assert_eq!(routes.validate(7, AudioDirection::Output, "output:0"), Ok(()));
}

#[test]
fn oversized_device_metadata_rejects_before_route_key_copying() {
    let mut snapshot = snapshot();
    snapshot.device.devices[0].label = "x".repeat(METADATA_BYTES);
    assert!(matches!(
        InventoryPreparer::draft(&snapshot),
        Err((7, RouteValidationError::Oversized))
    ));
}

#[test]
fn exported_catalog_handles_share_metadata_and_keep_the_original_quota() {
    let snapshot = snapshot();
    let pool = PayloadPool::new("inventory-export-test", 1, FRAME_BYTES).unwrap();
    let mut frame = InventoryPreparer::draft(&snapshot).unwrap();
    frame.sort();
    let bytes = frame.retained_bytes().unwrap();
    let view = DeviceView {
        frame: Some(pool.try_retain(frame, bytes).unwrap()),
        error: None,
        revision: 7,
    };
    assert!(view.catalog(8).is_none(), "stale metadata cannot supply fresh choices");
    let catalog = view.catalog(7).unwrap();
    let exported = catalog.clone();
    assert!(std::ptr::eq(catalog.state(), exported.state()));
    assert_eq!(catalog.state().devices.as_ptr(), exported.state().devices.as_ptr());
    assert_eq!(catalog.state(), &snapshot.device);
    drop((view, catalog));
    assert!(
        pool.try_reserve(1).is_err(),
        "catalog exports still own the same reservation"
    );
    drop(exported);
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(3)));
}
