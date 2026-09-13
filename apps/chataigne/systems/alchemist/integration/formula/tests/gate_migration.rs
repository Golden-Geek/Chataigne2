use golden_core::{
    app::ProjectLifecycle,
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

use crate::app::AppNode;

use super::{create_anode, engine_with_formula};
use crate::app::systems_alchemist_formula::{migrate_legacy_gate_semantics, GATE_SEMANTICS_V2_TAG};

#[test]
fn new_projects_mark_current_gate_semantics() {
    let root = AppNode::create_project_root();
    assert!(root.node_data().meta.tags.iter().any(|tag| tag == GATE_SEMANTICS_V2_TAG));
}

#[test]
fn historical_gate_modes_migrate_once_and_survive_sparse_reload() {
    let (mut engine, formula) = engine_with_formula();
    let cases = [
        ("pass_when_true", "output_default"),
        ("pass_when_false", "output_default_when_false"),
        ("hold_last", "hold_last_with_default"),
        ("block_trigger", "block_trigger_with_default"),
    ];
    let mut gates = Vec::new();
    for (index, (old, _)) in cases.iter().enumerate() {
        let gate = create_anode(&mut engine, formula, "condition_gate", index as f64, 0.0);
        let snapshot = engine.process_tree_snapshot();
        let config = snapshot.find_child_by_decl_id(gate, "config").unwrap();
        let mode = snapshot.find_child_by_decl_id(config, "config/mode").unwrap();
        let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
            node: mode,
            value: ParamValue::Enum((*old).to_owned()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        assert!(ack.success, "historical mode should be authored: {ack:?}");
        gates.push(engine.nodes.get(gate).unwrap().node_data().meta.uuid);
    }
    let project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project).unwrap();
    let before_history = loaded.undo_len();
    migrate_legacy_gate_semantics(&mut loaded).unwrap();
    assert_eq!(loaded.undo_len(), before_history, "load migration must not become a user undo step");
    let snapshot = loaded.process_tree_snapshot();
    assert!(snapshot.node(snapshot.root()).unwrap().tags.iter().any(|tag| tag == GATE_SEMANTICS_V2_TAG));
    for (uuid, (_, expected)) in gates.iter().zip(cases) {
        let gate = snapshot.node_id_by_uuid(*uuid).unwrap();
        let config = snapshot.find_child_by_decl_id(gate, "config").unwrap();
        let mode = snapshot.find_child_by_decl_id(config, "config/mode").unwrap();
        assert_eq!(snapshot.node(mode).unwrap().param_value, Some(ParamValue::Enum(expected.to_owned())));
    }
    migrate_legacy_gate_semantics(&mut loaded).unwrap();
    let migrated = golden_core::app::to_sparse_project_json_pretty(&loaded).unwrap();
    let reloaded = golden_core::app::from_sparse_project_json::<AppNode>(&migrated).unwrap();
    let snapshot = reloaded.process_tree_snapshot();
    assert!(snapshot.node(snapshot.root()).unwrap().tags.iter().any(|tag| tag == GATE_SEMANTICS_V2_TAG));
    for (uuid, (_, expected)) in gates.iter().zip(cases) {
        let gate = snapshot.node_id_by_uuid(*uuid).unwrap();
        let config = snapshot.find_child_by_decl_id(gate, "config").unwrap();
        let mode = snapshot.find_child_by_decl_id(config, "config/mode").unwrap();
        assert_eq!(snapshot.node(mode).unwrap().param_value, Some(ParamValue::Enum(expected.to_owned())));
    }
}

#[test]
fn unknown_historical_gate_mode_reports_error_without_marking_project() {
    let (mut engine, formula) = engine_with_formula();
    let gate = create_anode(&mut engine, formula, "condition_gate", 0.0, 0.0);
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot.find_child_by_decl_id(gate, "config").unwrap();
    let mode = snapshot.find_child_by_decl_id(config, "config/mode").unwrap();
    let AppNode::Parameter(parameter) = engine.nodes.get_mut(mode).unwrap() else {
        panic!("mode should be a parameter");
    };
    parameter.value = ParamValue::Enum("unrecognized".to_owned());
    let error = migrate_legacy_gate_semantics(&mut engine).unwrap_err();
    assert!(error.contains("unknown historical mode"));
    assert!(!engine.process_tree_snapshot().node(engine.root).unwrap().tags.iter().any(|tag| tag == GATE_SEMANTICS_V2_TAG));
}
