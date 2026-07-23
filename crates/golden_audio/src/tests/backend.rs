use crate::{
    AudioBackend, AudioBackendState, AudioBufferPolicy, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceId,
    AudioDeviceReadiness, AudioDeviceTargetId, AudioDirection, AudioError, AudioErrorCategory, AudioSampleFormat,
    AudioStreamStatus, BackendId, MockBackend, MockBackendEventKind, NullBackend, PhysicalChannelDescriptor,
    PhysicalChannelKey, SampleRate, StreamRequest, SupportedBufferFrames, SupportedStreamConfiguration,
    profile_key_for,
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
        profile_key: profile_key_for(&target, None, None),
        target: target.clone(),
        label: "Device 1".to_owned(),
        stable_id: true,
        fingerprint: AudioDeviceFingerprint {
            product: Some("fixture-1".to_owned()),
            output_channels: 1,
            ..AudioDeviceFingerprint::default()
        },
        input_channels: Vec::new(),
        output_channels: vec![PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new("output:0").unwrap(),
            label: "Output 1".to_owned(),
            position: None,
        }],
        supported_configurations: vec![SupportedStreamConfiguration {
            direction: AudioDirection::Output,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            min_sample_rate: 48_000,
            max_sample_rate: 48_000,
            buffer_frames: SupportedBufferFrames {
                min: 32,
                max: 1_024,
                preferred: 128,
            },
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

#[test]
fn mock_backend_models_hotplug_format_restart_permissions_and_flapping() {
    let backend_id = BackendId::new("recovery-backend").unwrap();
    let (backend, control) = MockBackend::new(backend_id.clone(), "Recovery Backend");
    let device = mock_output_device(backend_id, "device-recovery", true);
    let target = device.target.clone();
    control.connect_device(device.clone()).unwrap();
    assert!(matches!(
        control.drain_events().unwrap().as_slice(),
        [event] if matches!(event.kind, MockBackendEventKind::Connected(_))
    ));
    assert_eq!(backend.discover().unwrap().as_slice(), std::slice::from_ref(&device));

    control.set_default(AudioDirection::Output, &target).unwrap();
    let changed = SupportedStreamConfiguration {
        direction: AudioDirection::Output,
        channels: 1,
        sample_format: AudioSampleFormat::F32,
        min_sample_rate: 48_000,
        max_sample_rate: 96_000,
        buffer_frames: SupportedBufferFrames {
            min: 64,
            max: 512,
            preferred: 256,
        },
    };
    control
        .change_supported_configurations(&target, vec![changed.clone()])
        .unwrap();
    assert_eq!(backend.discover().unwrap()[0].supported_configurations, [changed]);

    let request = StreamRequest {
        direction: AudioDirection::Output,
        target: target.clone(),
        engine_sample_rate: SampleRate::new(48_000).unwrap(),
        channels: 1,
        buffer_policy: AudioBufferPolicy::Automatic,
    };
    for (category, message) in [
        (AudioErrorCategory::DeviceBusy, "busy"),
        (AudioErrorCategory::PermissionDenied, "denied"),
    ] {
        control.fail_open(Some(AudioError::new(category, message))).unwrap();
        assert_eq!(backend.open_stream(&request).unwrap_err().category, category);
    }
    control.fail_open(None).unwrap();

    control
        .set_state(AudioBackendState::MissingServer, Some("server stopped".to_owned()))
        .unwrap();
    assert!(backend.discover().is_err());
    control.restart_server().unwrap();
    assert_eq!(backend.discover().unwrap().len(), 1);

    control.set_flapping(true).unwrap();
    assert!(backend.discover().is_err());
    assert!(backend.discover().is_ok());
    control.set_flapping(false).unwrap();

    control.disconnect_device(&target).unwrap();
    assert!(backend.discover().unwrap().is_empty());
    let events = control.drain_events().unwrap();
    assert!(events.windows(2).all(|pair| pair[0].revision < pair[1].revision));
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, MockBackendEventKind::ServerRestarted))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, MockBackendEventKind::Disconnected(_)))
    );
}

fn mock_output_device(backend: BackendId, id: &str, is_system_default_output: bool) -> AudioDeviceDescriptor {
    let target = AudioDeviceTargetId::Device {
        backend,
        device: AudioDeviceId::new(id).unwrap(),
    };
    AudioDeviceDescriptor {
        profile_key: profile_key_for(&target, None, None),
        target,
        label: "Mock Device".to_owned(),
        stable_id: true,
        fingerprint: AudioDeviceFingerprint {
            product: Some("Mock Device".to_owned()),
            output_channels: 1,
            ..AudioDeviceFingerprint::default()
        },
        input_channels: Vec::new(),
        output_channels: vec![PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new("output:0").unwrap(),
            label: "Output 1".to_owned(),
            position: None,
        }],
        supported_configurations: vec![SupportedStreamConfiguration {
            direction: AudioDirection::Output,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            min_sample_rate: 48_000,
            max_sample_rate: 48_000,
            buffer_frames: SupportedBufferFrames {
                min: 32,
                max: 1_024,
                preferred: 128,
            },
        }],
        is_system_default_input: false,
        is_system_default_output,
    }
}
