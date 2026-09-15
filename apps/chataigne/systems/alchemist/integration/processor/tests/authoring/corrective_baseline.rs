use super::*;

use golden_core::{
    edit::{Edit, NodeTree},
    node::{DeclId, Node},
};

use crate::app::{InputSource, InputsManager, StateProcessorManagedRegions};
use crate::app::systems_alchemist_formula::{
    PROPERTIES_DECL_ID, PROPERTY_MANAGER_NODE_TYPE,
};
use crate::app::systems_alchemist_processor::PROCESSOR_FORMULA_SOURCE_DECL_ID;

#[test]
fn mapping_managers_are_direct_visible_authoring_nodes() {
    let (mut engine, _, processor) = mapping_engine();
    let snapshot = engine.process_tree_snapshot();
    assert!(
        snapshot
            .find_child_by_decl_id(processor, PROCESSOR_MANAGED_REGIONS_DECL_ID)
            .is_none(),
        "Mapping should not retain a hidden managed-region root"
    );

    let inputs = region(&engine, processor, "inputs");
    let inputs_node = snapshot.node(inputs).expect("Inputs manager should exist");
    assert_eq!(inputs_node.parent, Some(processor));
    assert!(inputs_node.presentation.show_in_inspector_content);
    let permissions = &engine.nodes.get(inputs).unwrap().node_data().meta.user_permissions;
    assert!(!permissions.can_remove_and_duplicate);
    assert!(!permissions.can_edit_name);

    let authored_input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.child_ids(inputs).contains(&authored_input));
    assert!(
        snapshot.find_child_by_decl_id(authored_input, "config").is_some(),
        "ordinary input items should expose their typed configuration descendants"
    );
}

#[test]
fn mapping_correction_baseline_preserves_tuple_reduction_and_queued_delivery() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let sink = source_param(&mut engine, "Tuple result", 0.0);
    let inputs = region(&engine, processor, "inputs");
    for (label, value) in [("X", 2.0), ("Y", 3.0), ("Z", 5.0)] {
        let source = source_param(&mut engine, label, value);
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
    }

    let filters = region(&engine, processor, "filters");
    let sum_type = engine
        .nodes
        .get(filters)
        .expect("Filters region should exist")
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}sum")))
        .map(|item| item.node_type)
        .expect("three Float inputs should expose Sum");
    create_item(&mut engine, filters, &sum_type);

    let outputs = region(&engine, processor, "outputs");
    create_parameter_output(&mut engine, outputs, sink);

    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let sink = snapshot.node_id_by_uuid(sink).expect("sink should remain addressable");
    assert_eq!(snapshot.node(sink).unwrap().param_value, Some(ParamValue::Float(10.0)));
}

#[test]
fn legacy_sidecar_migration_preserves_region_and_item_uuids() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let inputs = region(&engine, processor, "inputs");
    let anode = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    let snapshot = engine.process_tree_snapshot();
    let processor_uuid = snapshot.node(processor).unwrap().uuid;
    let region_uuid = snapshot.node(inputs).unwrap().uuid;
    let anode_uuid = snapshot.node(anode).unwrap().uuid;
    let formula_param = snapshot.find_child_by_decl_id(processor, "formula").unwrap();
    let source_param = snapshot
        .find_child_by_decl_id(processor, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .unwrap();
    let formula_value = snapshot.node(formula_param).unwrap().param_value.clone().unwrap();
    let source_value = snapshot.node(source_param).unwrap().param_value.clone().unwrap();

    engine.edits.push(Edit::SetParam {
        node: formula_param,
        value: ParamValue::Reference(NodeReference::empty()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: source_param,
        value: ParamValue::Str(String::new()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();

    let mut legacy_root = StateProcessorManagedRegions::new();
    legacy_root.node_data_mut().meta.decl_id =
        DeclId(PROCESSOR_MANAGED_REGIONS_DECL_ID.to_owned());
    legacy_root
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    let legacy_root_uuid = legacy_root.node_data().meta.uuid;
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(legacy_root),
        parent: processor,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let legacy_root = engine
        .process_tree_snapshot()
        .node_id_by_uuid(legacy_root_uuid)
        .unwrap();
    engine.edits.push(Edit::MoveNode {
        node: inputs,
        new_parent: legacy_root,
        new_prev_sibling: None,
    });

    let snapshot = engine.process_tree_snapshot();
    let mapping = snapshot.node_id_by_uuid(mapping_uuid).unwrap();
    let properties = snapshot.find_child_by_decl_id(mapping, PROPERTIES_DECL_ID).unwrap();
    let input_property = snapshot
        .child_ids(properties)
        .into_iter()
        .find(|property| {
            snapshot.node(*property).is_some_and(|node| {
                node.node_type == PROPERTY_MANAGER_NODE_TYPE
                    && snapshot
                        .find_child_by_decl_id(*property, "role")
                        .and_then(|role| snapshot.node(role))
                        .and_then(|role| role.param_value.as_ref())
                        .and_then(ParamValue::as_str)
                        .is_some_and(|role| role == "input")
            })
        })
        .unwrap();
    let input_property_node = snapshot.node(input_property).unwrap();
    let surface_decl = super::super::super::processor_surface_decl_id_for_source(
        input_property_node.uuid,
        &input_property_node.tags,
    );
    let mut legacy_surface = InputsManager::new();
    legacy_surface.node_data_mut().meta.decl_id = DeclId(surface_decl.clone());
    legacy_surface.node_data_mut().meta.label = "Inputs".to_owned();
    let legacy_surface_uuid = legacy_surface.node_data().meta.uuid;
    let legacy_input = InputSource::new();
    let legacy_input_uuid = legacy_input.node_data().meta.uuid;
    let mut legacy_surface_tree = NodeTree::new(legacy_surface);
    legacy_surface_tree.push_child(NodeTree::new(legacy_input).as_user_item());
    engine.edits.push(Edit::AddNodeTree {
        tree: legacy_surface_tree,
        parent: processor,
        prev_sibling: None,
    });
    engine.edits.push(Edit::SetParam {
        node: formula_param,
        value: formula_value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: source_param,
        value: source_value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    assert!(engine
        .process_tree_snapshot()
        .find_child_by_decl_id(processor, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .is_some());
    let legacy_project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut migrated = golden_core::app::from_sparse_project_json::<AppNode>(&legacy_project).unwrap();
    sync_external_formulas(&mut migrated).unwrap();
    for _ in 0..4 {
        migrated.apply_edits().unwrap();
        migrated.run_tick(Duration::from_millis(8)).unwrap();
    }

    let snapshot = migrated.process_tree_snapshot();
    let processor = snapshot.node_id_by_uuid(processor_uuid).unwrap();
    assert!(snapshot
        .find_child_by_decl_id(processor, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .is_none());
    assert!(snapshot.find_child_by_decl_id(processor, &surface_decl).is_none());
    assert!(snapshot.node_id_by_uuid(legacy_surface_uuid).is_none());
    let migrated_inputs = region(&migrated, processor, "inputs");
    assert_eq!(snapshot.node(migrated_inputs).unwrap().uuid, region_uuid);
    assert_eq!(snapshot.node_id_by_uuid(anode_uuid).and_then(|id| snapshot.node(id).and_then(|node| node.parent)), Some(migrated_inputs));
    assert_eq!(snapshot.node_id_by_uuid(legacy_input_uuid).and_then(|id| snapshot.node(id).and_then(|node| node.parent)), Some(migrated_inputs));
}
