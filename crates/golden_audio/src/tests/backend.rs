use crate::{
    AudioBackend, AudioBackendState, AudioBufferPolicy, AudioDeviceDescriptor, AudioDeviceId, AudioDeviceReadiness,
    AudioDeviceTargetId, AudioDirection, AudioError, AudioErrorCategory, AudioStreamStatus, BackendId, MockBackend,
    NullBackend, PhysicalChannelDescriptor, PhysicalChannelKey, SampleRate, StreamRequest,
};

#[test]
fn null_backend_discovers_opens_starts_and_stops() {
    let backend = NullBackend;
    let devices = backend.discover().unwrap();
    assert_eq!(devices.len(), 1);
    assert!(devices[0].supports(AudioDirection::Input));
    assert!(devices[0].supports(AudioDirection::Output));

    let request = StreamRequest {
        direction: AudioDirection::Output,
        target: devices[0].target.clone(),
        engine_sample_rate: SampleRate::new(48_000).unwrap(),
        channels: 2,
        buffer_policy: AudioBufferPolicy::Fixed(128),
    };
    let mut stream = backend.open_stream(&request).unwrap();
    stream.start().unwrap();
    assert_eq!(stream.status().readiness, AudioDeviceReadiness::Ready);
    stream.stop().unwrap();
    assert_eq!(
        stream.status(),
        AudioStreamStatus {
            readiness: AudioDeviceReadiness::Disabled,
            enabled: false,
            active_target: None,
            ..stream.status()
        }
    );
}

#[test]
fn mock_backend_supports_deterministic_hotplug_and_open_failures() {
    let backend_id = BackendId::new("test-backend").unwrap();
    let (backend, control) = MockBackend::new(backend_id.clone(), "Test Backend");
    let target = AudioDeviceTargetId::Device {
        backend: backend_id,
        device: AudioDeviceId::new("device-1").unwrap(),
    };
    let device = AudioDeviceDescriptor {
        target: target.clone(),
        label: "Device 1".to_owned(),
        fingerprint: Some("fixture-1".to_owned()),
        input_channels: Vec::new(),
        output_channels: vec![PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new("output:0").unwrap(),
            label: "Output 1".to_owned(),
            position: None,
        }],
        is_system_default_input: false,
        is_system_default_output: true,
    };
    control.set_devices(vec![device]).unwrap();
    assert_eq!(backend.discover().unwrap().len(), 1);

    let request = StreamRequest {
        direction: AudioDirection::Output,
        target,
        engine_sample_rate: SampleRate::new(48_000).unwrap(),
        channels: 1,
        buffer_policy: AudioBufferPolicy::Automatic,
    };
    backend.open_stream(&request).unwrap();

    control
        .fail_open(Some(AudioError::new(AudioErrorCategory::DeviceBusy, "fixture is busy")))
        .unwrap();
    assert_eq!(
        backend.open_stream(&request).unwrap_err().category,
        AudioErrorCategory::DeviceBusy
    );

    control
        .set_state(
            AudioBackendState::MissingServer,
            Some("fixture server stopped".to_owned()),
        )
        .unwrap();
    assert_eq!(
        backend.discover().unwrap_err().category,
        AudioErrorCategory::BackendUnavailable
    );
}
