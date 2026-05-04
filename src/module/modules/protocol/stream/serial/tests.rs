use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterConstraints, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::SerialModule;

#[test]
fn serial_parameters_keep_port_before_input_and_no_sender_folder() {
    let (engine, module_id) = create_serial_module();
    let connection_id =
        find_child_by_key(&engine, module_id, "connection").expect("serial module should have a connection folder");
    let parameters_id =
        find_child_by_key(&engine, module_id, "parameters").expect("serial module should have a parameters folder");

    let connection_labels = child_labels(&engine, connection_id);
    let parameter_labels = child_labels(&engine, parameters_id);
    assert!(
        connection_labels.contains(&"Port".to_string()),
        "serial Port should be a direct Connection child; children were {connection_labels:?}"
    );
    let port_pos = connection_labels.iter().position(|l| l == "Port").unwrap();
    let baud_pos = connection_labels.iter().position(|l| l == "Baud Rate").unwrap();
    assert!(
        port_pos < baud_pos,
        "serial Port should appear before Baud Rate in Connection; children were {connection_labels:?}"
    );
    assert_eq!(
        parameter_labels.first().map(String::as_str),
        Some("Processing"),
        "serial Processing folder should be first under Parameters; children were {parameter_labels:?}"
    );
    assert!(
        !parameter_labels.iter().any(|label| label == "Sender"),
        "serial should not materialize a Sender folder under Parameters; children were {parameter_labels:?}"
    );
}

#[test]
fn serial_input_and_output_capabilities_are_always_enabled() {
    let (engine, module_id) = create_serial_module();
    let processing_id =
        find_path(&engine, module_id, "parameters/processing").expect("serial Processing folder should exist");
    let processing = engine
        .nodes
        .get(processing_id)
        .expect("serial Processing folder should exist");

    assert!(
        processing.node_data().meta.enabled,
        "serial Processing folder should be enabled by default"
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "connection/can_receive"),
        Some(true),
        "serial should always report incoming capability"
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "connection/can_send"),
        Some(true),
        "serial should always report outgoing capability"
    );
}

#[test]
fn serial_processing_can_be_disabled_without_affecting_data_capabilities() {
    let (mut engine, module_id) = create_serial_module();
    let processing_id =
        find_path(&engine, module_id, "parameters/processing").expect("serial Processing folder should exist");

    set_node_enabled(&mut engine, processing_id, false);
    engine
        .apply_edits()
        .expect("test processing disable patch should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("serial module tick should apply processing disable");
    engine
        .apply_edits()
        .expect("serial processing disable edits should apply");

    let processing = engine
        .nodes
        .get(processing_id)
        .expect("serial Processing folder should exist");
    assert!(
        !processing.node_data().meta.enabled,
        "serial Processing folder should stay disabled"
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "connection/can_receive"),
        Some(true),
        "serial should still report incoming capability when Processing is disabled"
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "connection/can_send"),
        Some(true),
        "serial should still report outgoing capability when Processing is disabled"
    );
}

#[test]
fn serial_module_root_enable_toggle_stops_and_restarts_transport_while_recovering() {
    let (mut engine, module_id) = create_serial_module();
    let port_name_id = serial_module(&engine, module_id).port_name.id();

    allow_serial_port_variant(&mut engine, port_name_id, "missing-test-port");
    set_param(
        &mut engine,
        port_name_id,
        ParamValue::Enum("missing-test-port".to_string()),
    );
    settle_transport_state(&mut engine, "serial transport config should settle");

    let module = serial_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "serial module should start a transport handle even when the selected port is recovering"
    );
    assert!(
        module.last_transport_config.is_some(),
        "serial module should retain transport config while enabled"
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_transport_state(&mut engine, "serial module disable should settle");

    let module = serial_module(&engine, module_id);
    assert!(
        module.transport.is_none(),
        "serial module should stop the recovering transport as soon as the module root is disabled"
    );
    assert!(
        module.last_transport_config.is_none(),
        "serial module should clear cached transport config while disabled"
    );

    engine
        .run_tick(Duration::from_millis(20))
        .expect("disabled serial module tick should succeed");

    let module = serial_module(&engine, module_id);
    assert!(
        module.transport.is_none(),
        "serial module should stay disconnected while disabled instead of reconnecting in the background"
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_transport_state(&mut engine, "serial module re-enable should restart transport");

    let module = serial_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "serial module should recreate its transport after re-enable"
    );
    assert!(
        module.last_transport_config.is_some(),
        "serial module should restore transport config after re-enable"
    );
}

fn create_serial_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SerialModule::create().into(), None);
    engine.apply_edits().expect("serial module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("serial defaults should materialize");
    }
    engine.resolve().expect("serial runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("serial module should be attached under root");

    (engine, module_id)
}

fn serial_module(engine: &crate::app::AppEngine, module_id: NodeId) -> &SerialModule {
    let crate::app::AppNode::SerialModule(module) =
        engine.nodes.get(module_id).expect("serial module should exist")
    else {
        panic!("expected SerialModule node");
    };

    module
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn allow_serial_port_variant(engine: &mut crate::app::AppEngine, node: NodeId, variant: &str) {
    let snapshot = engine
        .nodes
        .get(node)
        .and_then(|candidate| candidate.engine_param_snapshot())
        .expect("serial port parameter should exist");
    let mut constraints = snapshot.constraints.clone();
    constraints.enum_options.push(ParameterEnumOption {
        variant_id: variant.to_string(),
        value: ParamValue::Enum(variant.to_string()),
        label: variant.to_string(),
        tags: Vec::new(),
        ordering: None,
    });
    set_param_constraints(engine, node, constraints);
    engine.apply_edits().expect("serial port test enum option should apply");
}

fn set_param_constraints(engine: &mut crate::app::AppEngine, node: NodeId, constraints: ParameterConstraints) {
    engine.edits.push(Edit::SetParamConstraints { node, constraints });
}

fn set_node_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    for segment in path.split('/') {
        if segment.trim().is_empty() {
            continue;
        }
        current = find_child_by_key(engine, current, segment)?;
    }
    Some(current)
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id)?;
        if node_key_matches(child.node_data(), key) {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn child_labels(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut labels = Vec::new();
    let mut current = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = current {
        let child = engine
            .nodes
            .get(child_id)
            .expect("child id from node links should exist");
        labels.push(child.node_data().meta.label.clone());
        current = child.node_data().next_sibling;
    }
    labels
}

fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
    node.meta.decl_id.0 == key
        || node.meta.decl_id.0.rsplit('/').next() == Some(key)
        || node.meta.short_name == key
        || node.meta.label == key
}

fn bool_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<bool> {
    let param_id = find_path(engine, start, path)?;
    let param = engine.nodes.get(param_id)?;
    match param.engine_param_snapshot()?.value {
        ParamValue::Bool(value) => Some(value),
        _ => None,
    }
}

fn settle_transport_state(engine: &mut crate::app::AppEngine, context: &str) {
    engine.apply_edits().expect(context);
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("transport edits should dispatch");
    engine.apply_edits().expect("transport event reactions should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("transport tick should succeed");
    engine.apply_edits().expect("transport edits should apply");
}
