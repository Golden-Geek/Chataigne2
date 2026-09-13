use super::*;

use golden_core::script::{ScriptNode, ScriptNodeConfig, ScriptSource};
use golden_core::ui_sync::NodeMetaPatch;

fn script_path_to(engine: &AppEngine, target: NodeId) -> String {
    let snapshot = engine.process_tree_snapshot();
    let mut node = target;
    let mut child_indices = Vec::new();
    while node != engine.root {
        let parent = snapshot.node(node).unwrap().parent.unwrap();
        let index = snapshot
            .child_ids(parent)
            .iter()
            .position(|candidate| *candidate == node)
            .unwrap();
        child_indices.push(index);
        node = parent;
    }
    child_indices.reverse();
    child_indices
        .into_iter()
        .fold("tree.root()".to_owned(), |path, index| {
            format!("{path}.getChild({index})")
        })
}

fn remap_mapping_engine() -> (AppEngine, NodeId, NodeId, NodeId, NodeId) {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source = source_param(&mut engine, "Input", 2.0);
    let sink = source_param(&mut engine, "Result", 0.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source)),
    );
    let filters = region(&engine, processor, "filters");
    let remap_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}remap@managed/0")))
        .map(|item| item.node_type)
        .expect("a Float input should expose Remap");
    let remap = create_item(&mut engine, filters, &remap_type);
    set_socket_default(&mut engine, remap, "in_max", ParamValue::Float(5.0));
    let snapshot = engine.process_tree_snapshot();
    let remap_inputs = snapshot.find_child_by_decl_id(remap, "inputs").unwrap();
    let maximum_socket = snapshot.find_child_by_decl_id(remap_inputs, "inputs/in_max").unwrap();
    let maximum = snapshot
        .find_child_by_decl_id(maximum_socket, "inputs/in_max/value")
        .unwrap();
    let outputs = region(&engine, processor, "outputs");
    let output = create_item(
        &mut engine,
        outputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"),
    );
    set_config(
        &mut engine,
        output,
        "target",
        ParamValue::Reference(NodeReference::new(sink)),
    );
    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(20)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let sink_node = snapshot.node_id_by_uuid(sink).unwrap();
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(0.4)));
    (engine, filters, remap, maximum, sink_node)
}

#[test]
fn script_edit_of_live_mapping_filter_bound_reprocesses_steady_source() {
    let (mut engine, _, _, maximum, sink_node) = remap_mapping_engine();
    let target = script_path_to(&engine, maximum);
    let text = format!(
        "script.setApiVersion(1); let done = false; function update() {{ if (done) return; {target}.value = 4; done = true; }}"
    );
    engine.add_node(
        ScriptNode::new(
            "Control Mapping",
            ScriptNodeConfig {
                source: ScriptSource::Inline { text },
            },
        )
        .into(),
        None,
    );
    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(20)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(maximum).unwrap().param_value, Some(ParamValue::Float(4.0)));
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(0.5)));
}

#[test]
fn activating_state_with_mapping_control_updates_another_mappings_live_filter() {
    let (mut engine, _, _, maximum, sink_node) = remap_mapping_engine();
    let snapshot = engine.process_tree_snapshot();
    let mapping = snapshot
        .child_ids(engine.root)
        .into_iter()
        .find(|node| snapshot.node(*node).is_some_and(|entry| entry.node_type == FormulaLibrary::NODE_TYPE))
        .and_then(|library| {
            snapshot.child_ids(library).into_iter().find(|node| {
                snapshot.node(*node).is_some_and(|entry| entry.node_type == AlchemistFormulaDefinition::NODE_TYPE && entry.label == "Mapping")
            })
        });
    let mapping_uuid = mapping
        .and_then(|node| snapshot.node(node).map(|entry| entry.uuid))
        .expect("built-in Mapping should remain in the library");
    let manager = snapshot
        .child_ids(engine.root)
        .into_iter()
        .find(|node| snapshot.node(*node).is_some_and(|entry| entry.node_type == StateMachineManager::NODE_TYPE))
        .unwrap();
    let before_states = snapshot.child_ids(manager);
    engine.add_user_item(StateMachineState::new().into(), Some(manager));
    engine.apply_edits().unwrap();
    let state = engine
        .process_tree_snapshot()
        .child_ids(manager)
        .into_iter()
        .find(|node| !before_states.contains(node))
        .unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: state,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    assert!(ack.success, "control State should start disabled: {ack:?}");
    engine.apply_edits().unwrap();
    let processors = engine.process_tree_snapshot().find_child_by_decl_id(state, "processors").unwrap();
    let controller = create_item(
        &mut engine,
        processors,
        &FormulaSourceRef::project_uuid(mapping_uuid).processor_create_type(),
    );
    let source = source_param(&mut engine, "State control value", 4.0);
    let inputs = region(&engine, controller, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(&mut engine, input, "source", ParamValue::Reference(NodeReference::new(source)));
    let outputs = region(&engine, controller, "outputs");
    let output = create_item(
        &mut engine,
        outputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"),
    );
    let maximum_uuid = engine.process_tree_snapshot().node(maximum).unwrap().uuid;
    set_config(&mut engine, output, "target", ParamValue::Reference(NodeReference::new(maximum_uuid)));

    for _ in 0..8 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(maximum).unwrap().param_value, Some(ParamValue::Float(5.0)));
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(0.4)));

    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: state,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    assert!(ack.success, "control State should activate: {ack:?}");
    engine.apply_edits().unwrap();
    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(maximum).unwrap().param_value, Some(ParamValue::Float(4.0)));
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(0.5)));
}

#[test]
fn reordering_mapping_filters_rebuilds_the_active_pipeline() {
    let (mut engine, filters, remap, _, sink_node) = remap_mapping_engine();
    let clamp_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}clamp@managed/0")))
        .map(|item| item.node_type)
        .expect("a Float chain should expose Clamp");
    let clamp = create_item(&mut engine, filters, &clamp_type);
    set_socket_default(&mut engine, clamp, "minimum", ParamValue::Float(0.0));
    set_socket_default(&mut engine, clamp, "maximum", ParamValue::Float(0.3));
    for _ in 0..8 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
        Some(ParamValue::Float(0.3))
    );

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: clamp,
        new_parent: filters,
        new_prev_sibling: None,
    });
    assert!(ack.success, "Clamp should move before Remap: {ack:?}");
    engine.apply_edits().unwrap();
    assert_eq!(engine.process_tree_snapshot().child_ids(filters), vec![clamp, remap]);
    for _ in 0..8 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
        Some(ParamValue::Float(0.3 / 5.0))
    );

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: clamp,
        new_parent: filters,
        new_prev_sibling: Some(remap),
    });
    assert!(ack.success, "Clamp should move after Remap: {ack:?}");
    engine.apply_edits().unwrap();
    assert_eq!(engine.process_tree_snapshot().child_ids(filters), vec![remap, clamp]);
    for _ in 0..8 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
        Some(ParamValue::Float(0.3))
    );
}
