use super::*;

use golden_core::{
    app::ProjectNode,
    events::EventKind,
    node::{PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID},
};

fn convert_mapping(engine: &mut AppEngine, processor: NodeId) -> NodeUuid {
    let snapshot = engine.process_tree_snapshot();
    let convert = snapshot
        .find_child_by_decl_id(processor, "convert_to_formula")
        .expect("Mapping should expose its conversion trigger");
    let acknowledgement = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: convert,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(
        acknowledgement.success,
        "Mapping conversion should apply: {acknowledgement:?}"
    );
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let formula = snapshot
        .find_child_by_decl_id(processor, "formula")
        .expect("processor should retain its Formula reference");
    let Some(ParamValue::Reference(reference)) = snapshot.node(formula).and_then(|node| node.param_value.as_ref())
    else {
        panic!("processor should reference its converted Formula");
    };
    reference.uuid()
}

struct ConvertedProductExpectation {
    processor_uuid: NodeUuid,
    formula_uuid: NodeUuid,
    curve_uuid: NodeUuid,
    curve_key_uuid: NodeUuid,
    curve_value_uuid: NodeUuid,
    command_uuid: NodeUuid,
    bindings: OutputBindingConfig,
    graph_nodes: usize,
    graph_edges: usize,
}

fn assert_converted_product_tree(engine: &AppEngine, expected: &ConvertedProductExpectation) {
    let snapshot = engine.process_tree_snapshot();
    let processor = snapshot
        .node_id_by_uuid(expected.processor_uuid)
        .expect("processor identity should survive persistence");
    let formula_ref = snapshot
        .find_child_by_decl_id(processor, "formula")
        .expect("processor should keep its Formula reference");
    assert_eq!(
        snapshot.node(formula_ref).unwrap().param_value,
        Some(ParamValue::Reference(NodeReference::new(
            expected.formula_uuid,
        )))
    );

    let formula_node = snapshot
        .node_id_by_uuid(expected.formula_uuid)
        .expect("converted project Formula should survive persistence");
    let formula =
        formula_from_snapshot(&snapshot, formula_node).expect("converted project Formula should remain materializable");
    assert_eq!(formula.graph.nodes().count(), expected.graph_nodes);
    assert_eq!(formula.graph.edges().count(), expected.graph_edges);

    let curve = snapshot
        .node_id_by_uuid(expected.curve_uuid)
        .expect("curve resource should keep its authored identity");
    assert_eq!(snapshot.node(curve).unwrap().node_type, "animation_curve");
    assert!(snapshot.node_id_by_uuid(expected.curve_key_uuid).is_some());
    let curve_value = snapshot
        .node_id_by_uuid(expected.curve_value_uuid)
        .expect("curve key value should remain addressable");
    assert_eq!(
        snapshot.node(curve_value).unwrap().param_value,
        Some(ParamValue::Float(2.0))
    );

    let command = snapshot
        .node_id_by_uuid(expected.command_uuid)
        .expect("concrete Mapping command should keep its authored identity");
    assert_eq!(
        mapping_output_binding_config(&snapshot, command).unwrap(),
        expected.bindings
    );
}

#[test]
fn configured_mapping_conversion_preserves_concrete_commands_resources_and_graph() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let processor_uuid = engine.process_tree_snapshot().node(processor).unwrap().uuid;
    let source_uuid = source_param(&mut engine, "Curve source", 0.5);
    let sink_uuid = source_param(&mut engine, "Curve sink", 0.0);

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
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );

    let filters = region(&engine, processor, "filters");
    let curve_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| {
            item.node_type
                .starts_with(&format!("{ANODE_CREATE_PREFIX}curve_remap@managed/"))
        })
        .map(|item| item.node_type)
        .expect("Curve Remap should be available to a scalar Mapping");
    let curve_filter = create_item(&mut engine, filters, &curve_type);
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot.find_child_by_decl_id(curve_filter, "config").unwrap();
    let curve = snapshot.find_child_by_decl_id(config, "config/curve").unwrap();
    let curve_uuid = snapshot.node(curve).unwrap().uuid;
    let curve_key = snapshot
        .child_ids(curve)
        .into_iter()
        .find(|key| {
            snapshot
                .node(*key)
                .is_some_and(|node| node.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE)
                && snapshot
                    .find_child_by_decl_id(*key, "position")
                    .and_then(|position| snapshot.node(position))
                    .and_then(|position| position.param_value.as_ref())
                    == Some(&ParamValue::Float(1.0))
        })
        .expect("default Curve Remap should expose its final key");
    let curve_key_uuid = snapshot.node(curve_key).unwrap().uuid;
    let curve_value = snapshot
        .find_child_by_decl_id(curve_key, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID)
        .unwrap();
    let curve_value_uuid = snapshot.node(curve_value).unwrap().uuid;
    drop(snapshot);
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: curve_value,
                value: ParamValue::Float(2.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success
    );

    let outputs = region(&engine, processor, "outputs");
    let command = create_parameter_output(&mut engine, outputs, sink_uuid);
    let command_uuid = engine.process_tree_snapshot().node(command).unwrap().uuid;
    let expected_bindings = mapping_output_binding_config(&engine.process_tree_snapshot(), command).unwrap();

    let source_formula = engine.process_tree_snapshot().node_id_by_uuid(mapping_uuid).unwrap();
    let source_graph = formula_from_snapshot(&engine.process_tree_snapshot(), source_formula).unwrap();
    let expected_graph_nodes = source_graph.graph.nodes().count();
    let expected_graph_edges = source_graph.graph.edges().count();
    assert!(
        expected_graph_nodes >= 3,
        "Mapping should contain its managed graph operations"
    );

    let expected = ConvertedProductExpectation {
        processor_uuid,
        formula_uuid: convert_mapping(&mut engine, processor),
        curve_uuid,
        curve_key_uuid,
        curve_value_uuid,
        command_uuid,
        bindings: expected_bindings,
        graph_nodes: expected_graph_nodes,
        graph_edges: expected_graph_edges,
    };
    assert_converted_product_tree(&engine, &expected);

    let full = engine
        .to_project_json_pretty_with(|node| node.project_encode_data())
        .expect("full project should serialize");
    let full =
        AppEngine::from_project_json_with(&full, AppNode::project_decode_node).expect("full project should reload");
    assert_converted_product_tree(&full, &expected);

    let sparse = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should serialize");
    let mut sparse =
        golden_core::app::from_sparse_project_json::<AppNode>(&sparse).expect("sparse project should reload");
    sync_external_formulas(&mut sparse).unwrap();
    assert_converted_product_tree(&sparse, &expected);
}

#[test]
fn action_command_copy_into_mapping_gets_local_bindings_without_stealing_source() {
    let (mut engine, _, processor) = mapping_engine();
    let mapping_outputs = region(&engine, processor, "outputs");
    let action_outputs = transitional_command_manager(&mut engine);
    let sink_uuid = source_param(&mut engine, "Cross-context sink", 0.0);
    let action_command = create_item(&mut engine, action_outputs, GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE);
    set_direct_child_param(
        &mut engine,
        action_command,
        "target",
        ParamValue::Reference(NodeReference::new(sink_uuid)),
    );
    let before = engine.process_tree_snapshot();
    let action_uuid = before.node(action_command).unwrap().uuid;
    assert!(
        before
            .find_child_by_decl_id(action_command, "mapping_bindings")
            .is_none()
    );
    let existing = before.child_ids(mapping_outputs);
    drop(before);

    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(
            ProjectFileSpec::new("Mapping copy", "mapping-copy"),
            None,
        ),
    );
    let acknowledgement = runtime
        .apply_ui_transaction(
            UiEditIntent::DuplicateNode {
                source: action_command,
                new_parent: mapping_outputs,
                new_prev_sibling: None,
                initial_params: Vec::new(),
            },
            Some("mapping-command-copy-test"),
        )
        .acknowledgement;
    assert!(
        acknowledgement.success,
        "Action command should copy into Mapping: {acknowledgement:?}"
    );
    for _ in 0..4 {
        runtime
            .run_tick(Duration::from_millis(8))
            .expect("copied command lifecycle should settle");
    }
    let snapshot = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let node = |id| {
        snapshot
            .nodes
            .iter()
            .find(|node| node.node_id == id)
            .expect("published node should exist")
    };
    let copied = node(mapping_outputs)
        .children
        .iter()
        .copied()
        .find(|node| !existing.contains(node))
        .expect("Mapping should contain the copied command");
    assert!(node(action_outputs).children.contains(&action_command));
    assert_eq!(node(action_command).uuid, action_uuid);
    assert_ne!(node(copied).uuid, action_uuid);
    assert!(
        node(action_command)
            .children
            .iter()
            .all(|child| node(*child).decl_id.0 != "mapping_bindings")
    );
    assert!(
        node(copied)
            .children
            .iter()
            .any(|child| node(*child).decl_id.0 == "mapping_bindings")
    );
    let target = node(copied)
        .children
        .iter()
        .copied()
        .find(|child| node(*child).decl_id.0 == "target")
        .expect("copied command should retain its target");
    let UiNodeDataDto::Parameter { param } = &node(target).data else {
        panic!("copied target should remain a parameter");
    };
    assert_eq!(
        param.value,
        ParamValue::Reference(NodeReference::new(sink_uuid))
    );
    let copied_uuid = node(copied).uuid;

    let undo = runtime
        .apply_ui_transaction(UiEditIntent::Undo, Some("mapping-command-copy-test"))
        .acknowledgement;
    assert!(undo.success, "copied command should undo: {undo:?}");
    let undone = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    assert!(undone.nodes.iter().all(|node| node.uuid != copied_uuid));
    assert!(undone.nodes.iter().any(|node| node.uuid == action_uuid));

    let redo = runtime
        .apply_ui_transaction(UiEditIntent::Redo, Some("mapping-command-copy-test"))
        .acknowledgement;
    assert!(redo.success, "copied command should redo: {redo:?}");
    for _ in 0..4 {
        runtime
            .run_tick(Duration::from_millis(8))
            .expect("redone command lifecycle should settle");
    }
    let redone = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let redone_copy = redone
        .nodes
        .iter()
        .find(|node| node.uuid == copied_uuid)
        .expect("redo should restore the copied command identity");
    assert!(redone_copy.children.iter().any(|child| {
        redone
            .nodes
            .iter()
            .find(|node| node.node_id == *child)
            .is_some_and(|node| node.decl_id.0 == "mapping_bindings")
    }));
}

#[test]
fn expanded_mapping_fanout_survives_live_add_remove_and_unrelated_modules() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source_uuid = source_param(&mut engine, "Live source", 1.0);
    let first_sink_uuid = source_param(&mut engine, "First sink", 0.0);
    let second_sink_uuid = source_param(&mut engine, "Second sink", 0.0);
    let third_sink_uuid = source_param(&mut engine, "Third sink", 0.0);
    let unrelated_uuid = source_param(&mut engine, "Unrelated module state", 42.0);
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
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs = region(&engine, processor, "outputs");
    let first = create_parameter_output(&mut engine, outputs, first_sink_uuid);
    let second = create_parameter_output(&mut engine, outputs, second_sink_uuid);

    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    for sink in [first_sink_uuid, second_sink_uuid] {
        let sink = snapshot.node_id_by_uuid(sink).unwrap();
        assert_eq!(snapshot.node(sink).unwrap().param_value, Some(ParamValue::Float(1.0)));
    }
    drop(snapshot);

    assert!(
        engine
            .apply_ui_intent(UiEditIntent::RemoveNode { node: second })
            .success
    );
    engine.apply_edits().unwrap();
    let third = create_parameter_output(&mut engine, outputs, third_sink_uuid);
    let source = engine.process_tree_snapshot().node_id_by_uuid(source_uuid).unwrap();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: source,
                value: ParamValue::Float(3.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success
    );
    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.node(first).is_some());
    assert!(snapshot.node(second).is_none());
    assert!(snapshot.node(third).is_some());
    for sink in [first_sink_uuid, third_sink_uuid] {
        let sink = snapshot.node_id_by_uuid(sink).unwrap();
        assert_eq!(snapshot.node(sink).unwrap().param_value, Some(ParamValue::Float(3.0)));
    }
    let removed_sink = snapshot.node_id_by_uuid(second_sink_uuid).unwrap();
    assert_eq!(
        snapshot.node(removed_sink).unwrap().param_value,
        Some(ParamValue::Float(1.0)),
        "removed commands must not receive later deliveries"
    );
    let unrelated = snapshot.node_id_by_uuid(unrelated_uuid).unwrap();
    assert_eq!(
        snapshot.node(unrelated).unwrap().param_value,
        Some(ParamValue::Float(42.0))
    );
    assert!(
        engine
            .ui_event_log()
            .iter()
            .filter(|event| { matches!(event.kind, EventKind::ParamChanged { param, .. } if param == removed_sink) })
            .count()
            <= 1,
        "removed fan-out must not duplicate effects"
    );
}
