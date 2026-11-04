use super::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    app::{
        ProjectLifecycle, ensure_preferences_defaults, insert_sparse_preferences_json, prepare_engine_for_runtime,
        to_sparse_preferences_json_pretty, to_sparse_project_json_pretty,
    },
    node::NodeId,
    parameter::{ParamValue, ParameterEnumOptions, ParameterEventBehaviour},
};
use std::time::Duration;

const INTEL: &str = "dxgi:00000000:00000001";
const NVIDIA: &str = "dxgi:00000000:00000002";
fn engine() -> AppEngine {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    ensure_preferences_defaults(&mut engine, AppNode::preferences_tree("folder".into()));
    prepare_engine_for_runtime(&mut engine).unwrap();
    engine
}
fn parameter(engine: &AppEngine) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let root = golden_core::app::live_preferences_root(engine).unwrap();
    let video = snapshot.find_child_by_decl_id(root, VIDEO_DECL_ID).unwrap();
    snapshot.find_child_by_decl_id(video, "adapter").unwrap()
}
fn drain(engine: &mut AppEngine) {
    engine.run_tick(Duration::from_millis(1)).unwrap();
}
fn publish(engine: &mut AppEngine, available: &[(String, String)]) {
    Source::capture(engine).unwrap().publish(available).unwrap();
    drain(engine);
}
fn available() -> Vec<(String, String)> {
    vec![(INTEL.into(), "Intel".into()), (NVIDIA.into(), "NVIDIA".into())]
}
fn set(engine: &mut AppEngine, value: &str) {
    let node = parameter(engine);
    let ack = golden_core::application::apply_project_intent(
        engine,
        golden_core::ui_sync::UiEditIntent::SetParam {
            node,
            value: ParamValue::Enum(value.into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
fn typed_gpu_preference_roundtrips_separately_and_missing_catalog_preserves_choice_and_history() {
    let mut engine = engine();
    publish(&mut engine, &available());
    set(&mut engine, INTEL);
    set(&mut engine, NVIDIA);
    let saved_id = engine.nodes.get(parameter(&engine)).unwrap().node_data().meta.uuid;
    let history = (engine.undo_len(), engine.redo_len());
    publish(&mut engine, &[]);
    assert_eq!(Source::capture(&engine).unwrap().value, NVIDIA);
    assert_eq!((engine.undo_len(), engine.redo_len()), history);
    assert!(engine.undo().unwrap());
    drain(&mut engine);
    assert_eq!(Source::capture(&engine).unwrap().value, INTEL);
    assert!(engine.redo().unwrap());
    drain(&mut engine);
    assert_eq!(Source::capture(&engine).unwrap().value, NVIDIA);
    assert_eq!(
        engine.nodes.get(parameter(&engine)).unwrap().node_data().meta.uuid,
        saved_id
    );
    let saved = to_sparse_preferences_json_pretty(&engine).unwrap().unwrap();
    assert!(
        !to_sparse_project_json_pretty(&engine)
            .unwrap()
            .contains("mapgyver.preferences.video")
    );
    let mut restored = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    insert_sparse_preferences_json(&mut restored, &saved).unwrap();
    restored.apply_edits().unwrap();
    // Subtree insertion remaps UUIDs into the new engine; merging defaults preserves those IDs.
    let loaded_id = restored.nodes.get(parameter(&restored)).unwrap().node_data().meta.uuid;
    ensure_preferences_defaults(&mut restored, AppNode::preferences_tree("fallback".into()));
    prepare_engine_for_runtime(&mut restored).unwrap();
    assert_eq!(Source::capture(&restored).unwrap().value, NVIDIA);
    assert_eq!(
        restored.nodes.get(parameter(&restored)).unwrap().node_data().meta.uuid,
        loaded_id
    );
    publish(&mut restored, &available());
    assert_eq!(Source::capture(&restored).unwrap().value, NVIDIA);
}

#[test]
fn metadata_is_latest_only_and_stale_values_or_retired_engine_cannot_accept_updates() {
    let mut engine = engine();
    let source = Source::capture(&engine).unwrap();
    source.publish(&[("first".into(), "First".into())]).unwrap();
    source.publish(&available()).unwrap();
    drain(&mut engine);
    let options = engine
        .nodes
        .get(parameter(&engine))
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options;
    assert!(!options.contains_variant("first"));
    set(&mut engine, NVIDIA);
    source.publish(&[]).unwrap();
    drain(&mut engine);
    assert_eq!(Source::capture(&engine).unwrap().value, NVIDIA);
    assert!(
        engine
            .nodes
            .get(parameter(&engine))
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .enum_options
            .contains_variant(NVIDIA)
    );
    drop(engine);
    assert!(source.publish(&available()).is_err());
}

#[test]
fn restored_missing_selection_gets_a_truthful_choice_without_changing_its_value() {
    let choices = catalog::choices(&available(), "dxgi:ffffffff:ffffffff", &ParameterEnumOptions::default()).unwrap();
    let missing = choices
        .as_slice()
        .iter()
        .find(|entry| entry.variant_id == "dxgi:ffffffff:ffffffff")
        .unwrap();
    assert!(missing.label.ends_with("(unavailable)"));
    assert_eq!(missing.value, ParamValue::Enum("dxgi:ffffffff:ffffffff".into()));
    let restored = catalog::choices(
        &[("dxgi:ffffffff:ffffffff".into(), "Returned GPU".into())],
        "dxgi:ffffffff:ffffffff",
        &choices,
    )
    .unwrap();
    assert_eq!(
        restored
            .as_slice()
            .iter()
            .find(|entry| entry.variant_id == "dxgi:ffffffff:ffffffff")
            .unwrap()
            .label,
        "Returned GPU"
    );
}
