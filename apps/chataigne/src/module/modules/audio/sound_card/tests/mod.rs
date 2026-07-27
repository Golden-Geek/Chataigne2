use std::sync::Arc;

use golden_audio::{
    AudioDeviceCatalogEntry, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceId,
    AudioDeviceInspectorState, AudioDeviceReadiness, AudioDeviceSelection, AudioDeviceTargetId, AudioDirection,
    AudioSampleFormat, BackendId, PhysicalChannelDescriptor, PhysicalChannelKey, SampleRate, SupportedBufferFrames,
    SupportedStreamConfiguration, profile_key_for,
};
use golden_core::{
    engine::EngineTime,
    events::{CustomEvent, Event, EventFrame},
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::{NO_AUDIO_DEVICE, SoundCardModule, find_path, integration, runtime};

#[test]
fn nested_connection_paths_and_flat_channel_parameters_materialize() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SoundCardModule::create().into(), None);
    for _ in 0..8 {
        engine
            .apply_edits()
            .expect("Sound Card declarations should materialize");
    }

    let module = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Sound Card module");
    let snapshot = engine.process_tree_snapshot();
    assert!(
        find_path(
            &snapshot,
            module,
            "connection/input_routing/routes"
        )
        .is_some()
    );
    assert!(
        find_path(
            &snapshot,
            module,
            "connection/output_routing/routes"
        )
        .is_some()
    );

    for path in ["parameters/input/channels", "parameters/output/channels"] {
        let channels = find_path(&snapshot, module, path).expect("channel container");
        let channel_ids = snapshot.child_ids_slice(channels);
        assert_eq!(channel_ids.len(), 2);
        for channel in channel_ids {
            let channel = snapshot.node(*channel).expect("channel gain");
            assert!(matches!(channel.param_value, Some(ParamValue::Float(_))));
            assert!(snapshot.child_ids_slice(channel.id).is_empty());
        }
    }
}

#[test]
fn device_selection_rebases_one_to_one_default_routes_without_accumulation() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SoundCardModule::create().into(), None);
    for _ in 0..8 {
        engine
            .apply_edits()
            .expect("Sound Card declarations should materialize");
    }
    let module = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Sound Card module");
    let input_channels = channels("input", 2)
        .into_iter()
        .map(|channel| channel.key)
        .collect::<Vec<_>>();
    let output_channels = channels("output", 2)
        .into_iter()
        .map(|channel| channel.key)
        .collect::<Vec<_>>();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    let sound_card = engine
        .nodes
        .get_mut(module)
        .expect("Sound Card module")
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .expect("Sound Card module type");
    sound_card.input_default_routes_pending = true;
    sound_card.output_default_routes_pending = true;
    assert!(sound_card.seed_default_routes_from_inventory(
        &mut ctx,
        &snapshot,
        &input_channels,
        &output_channels,
    ));
    for request in ctx.edits.drain() {
        engine.edits.push(request.edit);
    }
    engine
        .apply_edits()
        .expect("default routes should materialize");

    let snapshot = engine.process_tree_snapshot();
    for path in [
        "connection/input_routing/routes",
        "connection/output_routing/routes",
    ] {
        let routes = find_path(&snapshot, module, path).expect("route container");
        assert_eq!(snapshot.child_ids_slice(routes).len(), 2);
    }

    let mut reset_ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 2,
            micro: 0,
            seq: 0,
        },
    );
    let sound_card = engine
        .nodes
        .get_mut(module)
        .expect("Sound Card module")
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .expect("Sound Card module type");
    sound_card.input_default_routes_pending = true;
    sound_card.output_default_routes_pending = true;
    sound_card.input_route_reset_pending = true;
    sound_card.output_route_reset_pending = true;
    assert!(sound_card.reset_routes_for_device_selection(
        &mut reset_ctx,
        &snapshot,
    ));
    for request in reset_ctx.edits.drain() {
        engine.edits.push(request.edit);
    }
    engine
        .apply_edits()
        .expect("previous routes should be cleared");

    let snapshot = engine.process_tree_snapshot();
    for path in [
        "connection/input_routing/routes",
        "connection/output_routing/routes",
    ] {
        let routes = find_path(&snapshot, module, path).expect("route container");
        assert!(snapshot.child_ids_slice(routes).is_empty());
    }

    let mut reseed_ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 3,
            micro: 0,
            seq: 0,
        },
    );
    let sound_card = engine
        .nodes
        .get_mut(module)
        .expect("Sound Card module")
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .expect("Sound Card module type");
    assert!(sound_card.seed_default_routes_from_inventory(
        &mut reseed_ctx,
        &snapshot,
        &input_channels,
        &output_channels,
    ));
    for request in reseed_ctx.edits.drain() {
        engine.edits.push(request.edit);
    }
    engine
        .apply_edits()
        .expect("replacement routes should materialize");

    let snapshot = engine.process_tree_snapshot();
    for path in [
        "connection/input_routing/routes",
        "connection/output_routing/routes",
    ] {
        let routes = find_path(&snapshot, module, path).expect("route container");
        assert_eq!(snapshot.child_ids_slice(routes).len(), 2);
    }
}

#[test]
fn authored_routes_survive_sparse_project_save_and_load() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SoundCardModule::create().into(), None);
    for _ in 0..8 {
        engine
            .apply_edits()
            .expect("Sound Card declarations should materialize");
    }
    let module = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Sound Card module");
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = test_process_ctx(1);
    let sound_card = engine
        .nodes
        .get_mut(module)
        .expect("Sound Card module")
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .expect("Sound Card module type");
    sound_card.input_default_routes_pending = true;
    sound_card.output_default_routes_pending = true;
    assert!(sound_card.seed_default_routes_from_inventory(
        &mut ctx,
        &snapshot,
        &channels("input", 2)
            .into_iter()
            .map(|channel| channel.key)
            .collect::<Vec<_>>(),
        &channels("output", 2)
            .into_iter()
            .map(|channel| channel.key)
            .collect::<Vec<_>>(),
    ));
    for request in ctx.edits.drain() {
        engine.edits.push(request.edit);
    }
    engine
        .apply_edits()
        .expect("authored routes should materialize");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Sound Card project should serialize");
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
        .expect("Sound Card project should deserialize");
    let loaded_module = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded Sound Card module");
    let snapshot = loaded.process_tree_snapshot();
    for path in [
        "connection/input_routing/routes",
        "connection/output_routing/routes",
    ] {
        let routes = find_path(&snapshot, loaded_module, path).expect("decoded route container");
        assert_eq!(
            snapshot.child_ids_slice(routes).len(),
            2,
            "{path} should decode its authored routes before lifecycle reconciliation"
        );
    }
    loaded
        .apply_edits()
        .expect("loaded Sound Card declarations should reconcile");
    let snapshot = loaded.process_tree_snapshot();
    for path in [
        "connection/input_routing/routes",
        "connection/output_routing/routes",
    ] {
        let routes = find_path(&snapshot, loaded_module, path).expect("reconciled route container");
        assert_eq!(
            snapshot.child_ids_slice(routes).len(),
            2,
            "{path} should survive declared structure reconciliation"
        );
    }
    golden_core::app::prepare_engine_for_runtime(&mut loaded)
        .expect("loaded Sound Card should become ready");

    let loaded_module = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded Sound Card module");
    let snapshot = loaded.process_tree_snapshot();
    for path in [
        "connection/input_routing/routes",
        "connection/output_routing/routes",
    ] {
        let routes = find_path(&snapshot, loaded_module, path).expect("loaded route container");
        assert_eq!(
            snapshot.child_ids_slice(routes).len(),
            2,
            "{path} should retain its authored routes"
        );
    }
}

#[test]
fn telemetry_does_not_redrive_the_runtime_that_emitted_it() {
    let module = SoundCardModule::create();

    assert!(!module.inbox_drives_runtime(&custom_event_frame(
        chataigne_sound_card_protocol::SOUND_CARD_TELEMETRY_TOPIC,
    )));
    assert!(module.inbox_drives_runtime(&custom_event_frame(
        runtime::SOUND_CARD_RUNTIME_WAKE_TOPIC,
    )));
}

#[test]
fn unchanged_runtime_warnings_are_not_republished() {
    let mut module = SoundCardModule::create();
    let warning = runtime::RuntimeWarning {
        node: NodeId(42),
        id: "sound-card-unresolved-physical-channel".to_owned(),
        message: "unavailable channel".to_owned(),
        detail: None,
    };
    let mut first = test_process_ctx(1);
    module.apply_runtime_warnings(&mut first, std::slice::from_ref(&warning));
    assert_eq!(first.edits.pending.len(), 1);

    let mut unchanged = test_process_ctx(2);
    module.apply_runtime_warnings(&mut unchanged, std::slice::from_ref(&warning));
    assert!(unchanged.edits.pending.is_empty());

    let mut cleared = test_process_ctx(3);
    module.apply_runtime_warnings(&mut cleared, &[]);
    assert_eq!(cleared.edits.pending.len(), 1);
}

#[test]
fn sound_card_is_passive_and_no_driver_cannot_start_a_runtime() {
    let module = SoundCardModule::create();
    assert_eq!(module.execution_rule().update_rate, None);
    assert!(!module.needs_update());

    let error = runtime::SoundCardRuntime::start_selected(
        SampleRate::new(48_000).expect("valid sample rate"),
        None,
    )
    .expect_err("an explicit driver is required");
    assert!(error.contains("explicitly selected"));
}

#[test]
fn capability_intersection_uses_full_channel_configs_and_real_buffer_endpoints() {
    let backend = BackendId::new("fixture").expect("backend");
    let input = device(
        &backend,
        "input",
        "Input",
        2,
        0,
        vec![
            stream(AudioDirection::Input, 1, 8_000, 384_000, 32, 8_192, 128),
            stream(AudioDirection::Input, 2, 44_100, 48_000, 96, 300, 192),
        ],
    );
    let output = device(
        &backend,
        "output",
        "Output",
        0,
        2,
        vec![stream(
            AudioDirection::Output,
            2,
            48_000,
            96_000,
            128,
            384,
            160,
        )],
    );

    assert_eq!(
        runtime::supported_sample_rates(Some(&input), Some(&output), None),
        vec![48_000]
    );
    let buffers = runtime::supported_buffer_sizes(Some(&input), Some(&output), Some(48_000));
    assert!(buffers.contains(&160), "device-preferred non-power-of-two size");
    assert!(buffers.contains(&192), "other device's preferred size");
    assert!(buffers.contains(&300), "device range endpoint");
    assert!(!buffers.contains(&384), "outside the input-device intersection");

    let incompatible_output = device(
        &backend,
        "fixed-output",
        "Fixed Output",
        0,
        2,
        vec![stream(
            AudioDirection::Output,
            2,
            96_000,
            96_000,
            128,
            256,
            128,
        )],
    );
    assert!(
        runtime::supported_sample_rates(Some(&input), Some(&incompatible_output), None).is_empty()
    );
}

#[test]
fn asio_pairs_are_validated_before_probe_and_target_changes_require_disable() {
    let asio = BackendId::new("asio").expect("ASIO backend");
    let target_a = target(&asio, "a");
    let target_b = target(&asio, "b");
    let value_a = selection_value(&target_a, "A");
    let value_b = selection_value(&target_b, "B");

    assert!(
        runtime::selected_probe_targets(Some(&asio), &value_a, &value_b)
            .expect_err("different ASIO devices are invalid")
            .contains("same driver device")
    );
    assert_eq!(
        runtime::selected_probe_targets(Some(&asio), &value_a, &value_a).expect("same device"),
        vec![target_a.clone()]
    );

    let mut inspector = AudioDeviceInspectorState::default();
    inspector.input.active_target = Some(target_a.clone());
    assert!(!runtime::requires_disable_before_probe(
        Some(&asio),
        &inspector,
        std::slice::from_ref(&target_a),
    ));
    assert!(runtime::requires_disable_before_probe(
        Some(&asio),
        &inspector,
        std::slice::from_ref(&target_b),
    ));
    assert!(
        runtime::requires_disable_before_probe(
            Some(&asio),
            &inspector,
            &[target_a, target_b],
        ),
        "A/A to authored A/B must stop A before any exact probe",
    );
}

#[test]
fn asio_device_choices_expose_only_one_duplex_selector() {
    let asio = BackendId::new("asio").expect("ASIO backend");
    let duplex_target = target(&asio, "duplex");
    let output_only_target = target(&asio, "output-only");
    let state = AudioDeviceInspectorState {
        device_catalog: vec![
            AudioDeviceCatalogEntry {
                target: duplex_target.clone(),
                label: "Duplex Interface".to_owned(),
            },
            AudioDeviceCatalogEntry {
                target: output_only_target.clone(),
                label: "Output Only".to_owned(),
            },
        ],
        devices: vec![
            device(
                &asio,
                "duplex",
                "Duplex Interface",
                2,
                2,
                vec![
                    stream(AudioDirection::Input, 2, 48_000, 48_000, 64, 256, 128),
                    stream(AudioDirection::Output, 2, 48_000, 48_000, 64, 256, 128),
                ],
            ),
            device(
                &asio,
                "output-only",
                "Output Only",
                0,
                2,
                vec![stream(
                    AudioDirection::Output,
                    2,
                    48_000,
                    48_000,
                    64,
                    256,
                    128,
                )],
            ),
        ],
        ..AudioDeviceInspectorState::default()
    };

    let options = integration::duplex_device_options_for_current(NO_AUDIO_DEVICE, &state, Some(&asio));
    assert!(options.iter().any(|option| option.label == "Duplex Interface"));
    assert!(!options.iter().any(|option| option.label == "Output Only"));
    assert!(!options.iter().any(|option| option.label == "System Default"));
}

#[test]
fn module_connected_accepts_either_ready_direction() {
    let mut state = AudioDeviceInspectorState::default();
    state.input.enabled = true;
    state.input.readiness = AudioDeviceReadiness::Ready;
    state.output.enabled = true;
    state.output.readiness = AudioDeviceReadiness::Recovering;
    assert!(integration::any_direction_connected(&state));

    state.input.readiness = AudioDeviceReadiness::Recovering;
    assert!(!integration::any_direction_connected(&state));
}

#[test]
fn catalog_choices_are_available_before_probe_and_keep_friendly_authored_labels() {
    let backend = BackendId::new("fixture").expect("backend");
    let target = target(&backend, "interface");
    let entry = AudioDeviceCatalogEntry {
        target: target.clone(),
        label: "Live Interface".to_owned(),
    };
    let state = AudioDeviceInspectorState {
        device_catalog: vec![entry.clone()],
        ..AudioDeviceInspectorState::default()
    };
    let authored = selection_value(&target, "Previously Saved Name");

    let recovered = integration::device_options_for_current(
        authored.as_str(),
        &state,
        AudioDirection::Input,
        Some(&backend),
    );
    let recovered = recovered
        .iter()
        .find(|option| option.variant_id == authored)
        .expect("catalog target should retain the authored identity");
    assert_eq!(recovered.label, "Live Interface");
    assert_eq!(recovered.value, golden_core::parameter::ParamValue::Enum(authored.clone()));
    assert!(!recovered.tags.iter().any(|tag| tag == "missing"));

    let missing = integration::device_options_for_current(
        authored.as_str(),
        &AudioDeviceInspectorState::default(),
        AudioDirection::Input,
        Some(&backend),
    );
    let missing = missing
        .iter()
        .find(|option| option.variant_id == authored)
        .expect("missing authored choice");
    assert_eq!(missing.label, "Missing: Previously Saved Name");
    assert!(missing.tags.iter().any(|tag| tag == "missing"));

    let pending = runtime::selected_probe_targets(
        Some(&backend),
        authored.as_str(),
        NO_AUDIO_DEVICE,
    )
    .expect("catalog selection");
    assert!(runtime::probe_targets_are_pending(&state, &pending));
    let exact_state = AudioDeviceInspectorState {
        device_catalog: vec![entry],
        devices: vec![device(
            &backend,
            "interface",
            "Live Interface",
            1,
            1,
            vec![
                stream(AudioDirection::Input, 1, 48_000, 48_000, 64, 256, 128),
                stream(AudioDirection::Output, 1, 48_000, 48_000, 64, 256, 128),
            ],
        )],
        ..AudioDeviceInspectorState::default()
    };
    assert!(!runtime::probe_targets_are_pending(&exact_state, &pending));
}

fn target(backend: &BackendId, id: &str) -> AudioDeviceTargetId {
    AudioDeviceTargetId::Device {
        backend: backend.clone(),
        device: AudioDeviceId::new(id).expect("device id"),
    }
}

fn selection_value(target: &AudioDeviceTargetId, label: &str) -> String {
    let entry = AudioDeviceCatalogEntry {
        target: target.clone(),
        label: label.to_owned(),
    };
    runtime::device_selection_value(&AudioDeviceSelection::from_catalog_entry(&entry))
}

fn device(
    backend: &BackendId,
    id: &str,
    label: &str,
    input_channels: u16,
    output_channels: u16,
    supported_configurations: Vec<SupportedStreamConfiguration>,
) -> AudioDeviceDescriptor {
    let target = target(backend, id);
    let fingerprint = AudioDeviceFingerprint {
        input_channels,
        output_channels,
        ..AudioDeviceFingerprint::default()
    };
    AudioDeviceDescriptor {
        profile_key: profile_key_for(&target, None, None),
        target,
        label: label.to_owned(),
        stable_id: true,
        fingerprint,
        input_channels: channels("input", input_channels),
        output_channels: channels("output", output_channels),
        supported_configurations,
        is_system_default_input: false,
        is_system_default_output: false,
    }
}

fn channels(prefix: &str, count: u16) -> Vec<PhysicalChannelDescriptor> {
    (0..count)
        .map(|index| PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new(format!("{prefix}:{index}")).expect("physical channel"),
            label: format!("{prefix} {}", index + 1),
            position: None,
        })
        .collect()
}

fn stream(
    direction: AudioDirection,
    channels: u16,
    min_sample_rate: u32,
    max_sample_rate: u32,
    min_buffer: u32,
    max_buffer: u32,
    preferred_buffer: u32,
) -> SupportedStreamConfiguration {
    SupportedStreamConfiguration {
        direction,
        channels,
        sample_format: AudioSampleFormat::F32,
        min_sample_rate,
        max_sample_rate,
        buffer_frames: SupportedBufferFrames {
            min: min_buffer,
            max: max_buffer,
            preferred: preferred_buffer,
        },
    }
}

fn custom_event_frame(topic: &str) -> EventFrame {
    EventFrame::from_shared(vec![Arc::new(Event::custom(
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
        CustomEvent::latest(topic, None, serde_json::Value::Null),
    ))])
}

fn test_process_ctx(tick: u64) -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick,
            micro: 0,
            seq: 0,
        },
    )
}
