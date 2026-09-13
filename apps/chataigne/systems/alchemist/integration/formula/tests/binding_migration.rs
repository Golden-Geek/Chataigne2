use chataigne_state_machine::{
    MappingOutputBindingsDto, OutputArgumentBinding, OutputBindingConfig, OutputSendPolicy,
    OutputValueSource,
};
use chataigne_alchemist::{StableRef, ValueTypeId};
use golden_core::{
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};
use golden_values::Value as RuntimeValue;

use crate::app::AppNode;
use crate::app::systems_alchemist_formula::{
    migrate_output_binding_documents, OUTPUT_BINDINGS_V2_TAG,
};

use super::{create_anode, engine_with_formula};

fn bindings_param(engine: &crate::app::AppEngine, output: golden_core::node::NodeId) -> golden_core::node::NodeId {
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot.find_child_by_decl_id(output, "config").unwrap();
    snapshot.find_child_by_decl_id(config, "config/bindings").unwrap()
}

#[test]
fn historical_output_binding_document_migrates_once_and_survives_reload() {
    let (mut engine, formula) = engine_with_formula();
    let output = create_anode(&mut engine, formula, "chataigne.output_target", 0.0, 0.0);
    let output_uuid = engine.nodes.get(output).unwrap().node_data().meta.uuid;
    let config = OutputBindingConfig {
        value: OutputValueSource::Whole,
        arguments: vec![OutputArgumentBinding {
            parameter: StableRef::new(ValueTypeId::new("float"), "target/amount"),
            source: OutputValueSource::Constant(RuntimeValue::Float(2.5)),
        }],
        send_policy: OutputSendPolicy::OnChange,
    };
    let legacy = serde_json::to_string(&config).unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: bindings_param(&engine, output),
        value: ParamValue::Str(legacy),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "historical binding should be authored: {ack:?}");

    let project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project).unwrap();
    let undo_len = loaded.undo_len();
    migrate_output_binding_documents(&mut loaded).unwrap();
    assert_eq!(loaded.undo_len(), undo_len);
    let snapshot = loaded.process_tree_snapshot();
    assert!(snapshot.node(snapshot.root()).unwrap().tags.iter().any(|tag| tag == OUTPUT_BINDINGS_V2_TAG));
    let output = snapshot.node_id_by_uuid(output_uuid).unwrap();
    let document = snapshot.node(bindings_param(&loaded, output)).unwrap().param_value.as_ref().unwrap().as_str().unwrap();
    let authored: MappingOutputBindingsDto = serde_json::from_str(&document).unwrap();
    assert_eq!(authored.send_policy, chataigne_state_machine::MappingOutputSendPolicyDto::OnChange);
    assert_eq!(OutputBindingConfig::from_authoring_json(&document).unwrap(), config);

    migrate_output_binding_documents(&mut loaded).unwrap();
    let migrated = golden_core::app::to_sparse_project_json_pretty(&loaded).unwrap();
    let reloaded = golden_core::app::from_sparse_project_json::<AppNode>(&migrated).unwrap();
    let snapshot = reloaded.process_tree_snapshot();
    let output = snapshot.node_id_by_uuid(output_uuid).unwrap();
    let document = snapshot.node(bindings_param(&reloaded, output)).unwrap().param_value.as_ref().unwrap().as_str().unwrap();
    assert_eq!(OutputBindingConfig::from_authoring_json(&document).unwrap(), config);
}

#[test]
fn unknown_output_binding_schema_does_not_mark_project_migrated() {
    let (mut engine, formula) = engine_with_formula();
    let output = create_anode(&mut engine, formula, "chataigne.output_target", 0.0, 0.0);
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: bindings_param(&engine, output),
        value: ParamValue::Str(r#"{"unknown":true}"#.to_owned()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success);
    let error = migrate_output_binding_documents(&mut engine).unwrap_err();
    assert!(error.contains("unknown binding schema"));
    let snapshot = engine.process_tree_snapshot();
    assert!(!snapshot.node(snapshot.root()).unwrap().tags.iter().any(|tag| tag == OUTPUT_BINDINGS_V2_TAG));
}
