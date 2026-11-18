use super::{
    configuration,
    preferences::{Publication, Settings, Status},
};
use crate::app::{AppEngine, AppNode};
use golden_audio::{AudioBackend, AudioDeviceInspectorState, NullBackend};
use golden_core::{
    app::{
        ProjectLifecycle, ensure_preferences_defaults, insert_sparse_preferences_json, prepare_engine_for_runtime,
        to_sparse_preferences_json_pretty, to_sparse_project_json_pretty,
    },
    application::apply_project_intent,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};
use std::time::Duration;

fn fixture(saved: Option<&str>) -> AppEngine {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    if let Some(saved) = saved {
        insert_sparse_preferences_json(&mut engine, saved).unwrap();
        engine.apply_edits().unwrap();
    }
    ensure_preferences_defaults(&mut engine, AppNode::preferences_tree("test".into()));
    prepare_engine_for_runtime(&mut engine).unwrap();
    engine
}

fn child(engine: &AppEngine, parent: NodeId, declaration: &str) -> NodeId {
    engine
        .process_tree_snapshot()
        .find_child_by_decl_id(parent, declaration)
        .unwrap()
}

fn audio(engine: &AppEngine) -> NodeId {
    child(
        engine,
        golden_core::app::live_preferences_root(engine).unwrap(),
        "mapgyver_audio",
    )
}

fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
fn physical_output_layout_changes_restore_the_same_authored_channel_keys() {
    let mut engine = fixture(None);
    let left = child(&engine, audio(&engine), "main-left");
    let physical = child(&engine, left, "physical");
    set(&mut engine, physical, ParamValue::Enum("output:1".into()));
    let mut inventory = AudioDeviceInspectorState {
        devices: NullBackend.device_inventory().unwrap().devices,
        ..Default::default()
    };
    let backend = NullBackend::backend_id();
    let saved = to_sparse_preferences_json_pretty(&engine).unwrap().unwrap();
    assert!(
        !to_sparse_project_json_pretty(&engine)
            .unwrap()
            .contains("mapgyver.preferences.audio")
    );
    let history = (engine.undo_len(), engine.redo_len());
    let settings = Settings::capture(&engine).unwrap();
    let (config, _) = configuration::resolve(&settings, Some(&backend), &inventory).unwrap();
    assert_eq!(config.output_patch.len(), 2);
    config.validate(&Default::default()).unwrap();
    for (index, route) in config.playback_patch.iter().enumerate() {
        assert_eq!(usize::from(route.source_channel), index);
        assert_eq!(route.destination, config.virtual_outputs[index].id);
    }
    assert!(
        config
            .output_patch
            .iter()
            .all(|route| route.destination.as_str() == "output:1")
    );

    let missing = inventory.devices[0].output_channels.pop().unwrap();
    let (narrowed, choices) = configuration::resolve(&settings, Some(&backend), &inventory).unwrap();
    assert!(narrowed.output_patch.is_empty());
    settings.publish(Publication {
        status: Status::default(),
        choices,
    });
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let param = engine.nodes.get(physical).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(param.value, ParamValue::Enum("output:1".into()));
    assert!(
        param
            .constraints
            .enum_options
            .as_slice()
            .iter()
            .any(|option| option.variant_id == "output:1" && option.label.ends_with("(unavailable)"))
    );
    assert_eq!((engine.undo_len(), engine.redo_len()), history);

    inventory.devices[0].output_channels.insert(0, missing);
    let loaded = fixture(Some(&saved));
    let settings = Settings::capture(&loaded).unwrap();
    let (recovered, _) = configuration::resolve(&settings, Some(&backend), &inventory).unwrap();
    assert_eq!(recovered.virtual_outputs, config.virtual_outputs);
    assert_eq!(recovered.playback_patch, config.playback_patch);
    assert_eq!(recovered.output_patch, config.output_patch);
    assert_eq!(recovered.physical_outputs[0].as_str(), "output:1");
}

#[test]
fn an_explicit_missing_audio_device_keeps_its_exact_target() {
    let mut engine = fixture(None);
    let preferences = audio(&engine);
    let device = child(&engine, preferences, "device");
    let enabled = child(&engine, preferences, "output_enabled");
    set(&mut engine, enabled, ParamValue::Bool(true));
    set(&mut engine, device, ParamValue::Enum("wasapi/test-device".into()));
    let settings = Settings::capture(&engine).unwrap();
    let (config, _) = configuration::resolve(&settings, Some(&NullBackend::backend_id()), &Default::default()).unwrap();
    config.validate(&Default::default()).unwrap();
    assert!(settings.enabled);
    assert!(!config.output.enabled);
    let selection = config.output.device.unwrap();
    assert_eq!(selection.target.backend().as_str(), "wasapi");
    assert_eq!(
        config.output.recovery_policy,
        golden_audio::AudioRecoveryPolicy::WaitForSelected
    );
    assert_eq!(config.virtual_outputs.len(), 2);
    assert!(config.output_patch.is_empty());
}

#[test]
fn a_duplicated_virtual_output_gets_a_new_identity_that_survives_reload() {
    let mut engine = fixture(None);
    let parent = audio(&engine);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: "mapgyver.preferences.audio.output".into(),
            label: Some("Auxiliary output".into()),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let source = ack.created_node.unwrap().node_id;
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source,
            new_parent: parent,
            new_prev_sibling: Some(source),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");

    let ids = |engine: &AppEngine| {
        Settings::capture(engine)
            .unwrap()
            .outputs
            .into_iter()
            .map(|output| output.key.parse::<golden_audio::AudioChannelId>().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let original = ids(&engine);
    assert_eq!(original.len(), 4);
    let saved = to_sparse_preferences_json_pretty(&engine).unwrap().unwrap();
    assert_eq!(ids(&fixture(Some(&saved))), original);
}
