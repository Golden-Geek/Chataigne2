use super::*;

#[test]
fn explicit_output_none_survives_sparse_project_round_trip() {
    let (mut engine, module) = create_sound_card();
    select_enum_parameter(
        &mut engine,
        module,
        "connection/output_device",
        NO_AUDIO_DEVICE,
    );

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Sound Card project should serialize");
    let mut loaded =
        golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
            .expect("Sound Card project should deserialize");
    let loaded_module = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded Sound Card module");
    assert_selector_value(
        &loaded.process_tree_snapshot(),
        loaded_module,
        "connection/output_device",
        NO_AUDIO_DEVICE,
    );

    loaded
        .apply_edits()
        .expect("loaded Sound Card declarations should reconcile");
    assert_selector_value(
        &loaded.process_tree_snapshot(),
        loaded_module,
        "connection/output_device",
        NO_AUDIO_DEVICE,
    );
}

#[test]
fn missing_output_selector_cannot_fall_back_to_system_default_hardware() {
    let (mut engine, module) = create_sound_card();
    let snapshot = engine.process_tree_snapshot();
    let output =
        find_path(&snapshot, module, "connection/output_device").expect("output selector");
    engine.edits.push(Edit::RemoveNode { node: output });
    engine
        .apply_edits()
        .expect("output selector removal fixture should apply");

    let error = runtime::build_configuration(
        &engine.process_tree_snapshot(),
        module,
        &AudioDeviceInspectorState::default(),
    )
    .expect_err("a structurally missing selector must reject the configuration");
    assert!(
        error.message.contains("output_device"),
        "the structural error should identify the missing selector: {}",
        error.message,
    );
}

#[test]
fn exact_input_with_authored_output_none_builds_input_only_configuration() {
    let (mut engine, module) = create_sound_card();
    let backend = BackendId::from_static("null");
    let input = device(
        &backend,
        "bluetooth-input",
        "Bluetooth Headset",
        2,
        0,
        vec![stream(
            AudioDirection::Input,
            2,
            48_000,
            48_000,
            64,
            2_048,
            480,
        )],
    );
    let selected_input = selection_value(&input.target, input.label.as_str());
    select_enum_parameter(
        &mut engine,
        module,
        "connection/output_device",
        NO_AUDIO_DEVICE,
    );
    select_enum_parameter(
        &mut engine,
        module,
        "connection/input_device",
        selected_input.as_str(),
    );

    let inspector = AudioDeviceInspectorState {
        device_catalog: vec![AudioDeviceCatalogEntry::from(&input)],
        devices: vec![input.clone()],
        ..AudioDeviceInspectorState::default()
    };
    let built = runtime::build_configuration(
        &engine.process_tree_snapshot(),
        module,
        &inspector,
    )
    .expect("exact input and disabled output should build");

    assert!(built.configuration.input.enabled);
    assert_eq!(
        built
            .configuration
            .input
            .device
            .as_ref()
            .map(|selection| &selection.target),
        Some(&input.target),
    );
    assert!(!built.configuration.output.enabled);
    assert!(built.configuration.output.device.is_none());
    assert!(built.configuration.physical_outputs.is_empty());
}

#[test]
fn stream_status_events_do_not_resubmit_the_active_configuration() {
    let mut status = AudioStreamStatus::disabled(AudioDirection::Output);
    status.enabled = true;
    status.readiness = AudioDeviceReadiness::Ready;

    assert!(!integration::runtime_event_changes_inventory(
        &golden_audio::AudioEvent::DeviceStatusChanged(status),
    ));
    assert!(!integration::runtime_event_changes_inventory(
        &golden_audio::AudioEvent::BackendStatusChanged(
            golden_audio::AudioBackendStatus {
                backend: BackendId::from_static("fixture"),
                label: "Fixture".to_owned(),
                state: golden_audio::AudioBackendState::Available,
                detail: None,
            },
        ),
    ));
    assert!(integration::runtime_event_changes_inventory(
        &golden_audio::AudioEvent::DeviceInventoryChanged { revision: 2 },
    ));
}

#[test]
fn device_connection_feedback_covers_progress_success_failure_and_warning_recovery() {
    let backend = BackendId::from_static("fixture");
    let mut status = AudioStreamStatus::disabled(AudioDirection::Output);
    status.enabled = true;
    status.selected_target = Some(AudioDeviceTargetId::SystemDefault {
        backend: backend.clone(),
    });
    status.profile_key = Some(golden_audio::profile_key_for(
        status.selected_target.as_ref().expect("selected target"),
        None,
        Some(AudioDirection::Output),
    ));
    status.selected_label = Some("Studio Interface".to_owned());
    status.readiness = AudioDeviceReadiness::Discovering;

    assert!(connection_feedback::status_matches_selection(
        &status,
        SYSTEM_DEFAULT_DEVICE,
        Some(&backend),
    ));
    assert!(
        !connection_feedback::status_matches_selection(
            &status,
            NO_AUDIO_DEVICE,
            Some(&backend),
        ),
        "queued output status must become stale as soon as Output Device is None",
    );
    assert!(
        !connection_feedback::status_matches_selection(
            &status,
            SYSTEM_DEFAULT_DEVICE,
            Some(&BackendId::from_static("other")),
        ),
        "queued status from a previous driver must not produce feedback",
    );

    let connecting = connection_feedback::log_for_status(&status).expect("connecting log");
    assert_eq!(
        connecting.level,
        connection_feedback::ConnectionLogLevel::Info
    );
    assert_eq!(
        connecting.message,
        "Connecting to output audio device [Studio Interface]..."
    );
    assert_eq!(
        connection_feedback::warning_for_status(&status),
        connection_feedback::ConnectionWarning::Keep
    );

    status.readiness = AudioDeviceReadiness::Preparing;
    assert!(
        connection_feedback::log_for_status(&status).is_none(),
        "one connection attempt should emit only one progress log",
    );

    status.readiness = AudioDeviceReadiness::Ready;
    let connected = connection_feedback::log_for_status(&status).expect("connected log");
    assert_eq!(
        connected.level,
        connection_feedback::ConnectionLogLevel::Success
    );
    assert_eq!(
        connection_feedback::warning_for_status(&status),
        connection_feedback::ConnectionWarning::Clear
    );

    status.readiness = AudioDeviceReadiness::Busy;
    status.error = Some(AudioInspectorError {
        category: AudioErrorCategory::DeviceBusy,
        message: "device is already in use".to_owned(),
        technical_detail: Some("host: test".to_owned()),
    });
    let failed = connection_feedback::log_for_status(&status).expect("failure log");
    assert_eq!(
        failed.level,
        connection_feedback::ConnectionLogLevel::Error
    );
    assert!(failed.message.contains("device is already in use"));
    assert_eq!(
        connection_feedback::warning_for_status(&status),
        connection_feedback::ConnectionWarning::Set {
            message: "Could not connect to output audio device [Studio Interface]".to_owned(),
            detail: "device is already in use\nhost: test".to_owned(),
        }
    );

    status.readiness = AudioDeviceReadiness::Recovering;
    assert_eq!(
        connection_feedback::warning_for_status(&status),
        connection_feedback::ConnectionWarning::Keep,
        "the persistent warning must survive automatic reconnect attempts",
    );

    let mut input_failure = AudioStreamStatus::disabled(AudioDirection::Input);
    input_failure.enabled = true;
    input_failure.selected_label = Some("Bluetooth Headset".to_owned());
    input_failure.readiness = AudioDeviceReadiness::Failed;
    input_failure.error = Some(AudioInspectorError {
        category: AudioErrorCategory::UnsupportedFormat,
        message: "stream configuration is not supported".to_owned(),
        technical_detail: None,
    });
    let input_log =
        connection_feedback::log_for_status(&input_failure).expect("input failure log");
    assert!(
        input_log
            .message
            .starts_with("Could not connect to input audio device [Bluetooth Headset]"),
        "input failures must be attributed to the input selector: {}",
        input_log.message,
    );
}
