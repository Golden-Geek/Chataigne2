use golden_core::{
    node::{Folder, Node, NodeId, NodeReference},
    parameter::{ParamValue, Parameter},
    process_ctx::ExecutionPhase,
    ui_sync::UiEditIntent,
};

use crate::app::{
    AlchemistFormulaDefinition, AppEngine, AppNode, ConditionManager,
    ConsequencesManager, FilterChainManager, FormulaLibrary, InputsManager,
    OutputsManager,
};

use super::{
    PROCESSOR_FOLDER_ITEM_KIND, PROCESSOR_FOLDER_NODE_TYPE,
    PROCESSOR_ITEM_KIND, StateProcessor, StateProcessorFolder,
    StateProcessorManager, StateProcessorProperties,
};
use crate::app::state_machine_nodes_formula::{
    AlchemistProperty, PROPERTIES_DECL_ID, PROPERTY_CREATE_PREFIX,
    PROPERTY_FOLDER_NODE_TYPE, PROPERTY_MANAGER_CREATE_PREFIX,
};

fn engine_with_formula() -> (
    AppEngine,
    NodeId,
    golden_core::node::NodeUuid,
) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("formula library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Formula Library should exist");
    let mut formula = <AlchemistFormulaDefinition as Node>::project_create(
        AlchemistFormulaDefinition::NODE_TYPE,
    )
    .expect("custom Formula should be creatable");
    formula.node_data_mut().meta.label = "Custom Formula".into();
    let formula_uuid = formula.node_data().meta.uuid;
    engine.add_user_item(formula.into(), Some(library));
    engine.apply_edits().expect("formula should attach");
    let formula_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.uuid == formula_uuid)
        .map(|(id, _)| id)
        .expect("Formula should attach");
    (engine, formula_id, formula_uuid)
}

#[test]
fn processor_manager_lists_custom_formulas() {
    let (mut engine, _, _) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let manager = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(_, node)| node)
        .expect("processor manager should exist");
    let formula_items = manager
        .user_creatable_items()
        .into_iter()
        .filter(|item| item.item_kind == PROCESSOR_ITEM_KIND)
        .collect::<Vec<_>>();

    assert_eq!(formula_items.len(), 1);
    assert_eq!(formula_items[0].label, "Custom Formula");
    assert!(formula_items[0].node_type.starts_with("state_processor:"));
}

#[test]
fn processor_created_from_formula_item_keeps_a_stable_reference() {
    let (mut engine, _, formula_uuid) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let formula_param = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().parent == Some(processor_id))
        .find_map(|(_, node)| node.as_any().downcast_ref::<Parameter>())
        .expect("Processor should have a Formula reference");

    assert!(matches!(
        &formula_param.value,
        ParamValue::Reference(reference) if reference.uuid() == formula_uuid
    ));
}

#[test]
fn processor_folder_accepts_processors_and_folders() {
    let manager = StateProcessorManager::new();
    let folder = StateProcessorFolder::new();
    assert!(manager.user_container_accepts_item(StateProcessor::NODE_TYPE, PROCESSOR_ITEM_KIND));
    assert!(manager.user_container_accepts_item(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND
    ));
    assert!(folder.user_container_accepts_item(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND
    ));
    assert!(folder
        .create_user_item(PROCESSOR_FOLDER_NODE_TYPE)
        .is_some());
    assert!(folder.user_container_accepts_item(
        &format!("state_processor:{}", uuid::Uuid::nil()),
        PROCESSOR_ITEM_KIND
    ));
    assert!(!manager.user_container_accepts_item("module", "module"));
    assert!(!manager.user_container_accepts_item("module_folder", "module_folder"));
    assert!(!folder.user_container_accepts_item("folder", PROCESSOR_FOLDER_ITEM_KIND));
    assert!(!folder.user_container_accepts_item("state", "state"));
}

#[test]
fn processor_nodes_do_not_accept_nested_processors() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");

    engine.add_user_item(StateProcessor::new().into(), Some(manager_id));
    engine.add_user_item(StateProcessor::new().into(), Some(manager_id));
    engine
        .apply_edits()
        .expect("processors should attach to the manager");

    let processors = engine
        .process_tree_snapshot()
        .child_ids(manager_id)
        .into_iter()
        .filter(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|node| node.get_type() == StateProcessor::NODE_TYPE)
        })
        .collect::<Vec<_>>();
    assert_eq!(processors.len(), 2);

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: processors[1],
        new_parent: processors[0],
        new_prev_sibling: None,
    });
    assert!(
        !ack.success,
        "processor nodes must not accept nested processor drops"
    );
}

#[test]
fn processor_materializes_formula_properties_as_real_children() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    let properties = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        .expect("Formula Properties should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
        label: Some("Amount".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Formula parameter creation should succeed: {ack:?}");
    let formula_property = engine
        .process_tree_snapshot()
        .child_ids(properties)
        .into_iter()
        .find(|child| {
            engine.nodes.get(*child).is_some_and(|node| {
                node.get_type() == AlchemistProperty::NODE_TYPE
            })
        })
        .expect("Formula parameter should exist");
    for (role, label) in [
        ("condition", "Conditions"),
        ("consequence", "Consequences"),
        ("input", "Inputs"),
        ("filter", "Filters"),
        ("output", "Outputs"),
    ] {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: properties,
            node_type: format!("{PROPERTY_MANAGER_CREATE_PREFIX}{role}"),
            label: Some(label.into()),
            initial_params: Vec::new(),
        });
        assert!(
            ack.success,
            "Formula {role} manager creation should succeed: {ack:?}"
        );
    }

    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let snapshot = engine.process_tree_snapshot();
    let instance_properties = snapshot
        .find_child_by_decl_id(processor_id, PROPERTIES_DECL_ID)
        .expect("Processor should instantiate Formula Properties");
    assert_eq!(
        engine
            .nodes
            .get(instance_properties)
            .expect("Processor Properties should exist")
            .get_type(),
        StateProcessorProperties::NODE_TYPE
    );
    let instance_children = snapshot.child_ids(instance_properties);
    let parameter = instance_children
        .iter()
        .copied()
        .find(|child| {
            engine.nodes.get(*child).is_some_and(|node| {
                node.engine_param_snapshot().is_some()
                    && node.node_data().meta.label == "Amount"
            })
        })
        .expect("Processor should own an editable Amount parameter");
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .and_then(Node::engine_param_snapshot)
            .map(|parameter| parameter.value),
        Some(ParamValue::Float(0.0))
    );
    for (node_type, label) in [
        (ConditionManager::NODE_TYPE, "Conditions"),
        (ConsequencesManager::NODE_TYPE, "Consequences"),
        (InputsManager::NODE_TYPE, "Inputs"),
        (FilterChainManager::NODE_TYPE, "Filters"),
        (OutputsManager::NODE_TYPE, "Outputs"),
    ] {
        assert!(instance_children.iter().copied().any(|child| {
            engine.nodes.get(child).is_some_and(|node| {
                node.get_type() == node_type
                    && node.node_data().meta.label == label
            })
        }));
    }
    let source_uuid = engine
        .nodes
        .get(formula_property)
        .expect("Formula parameter should exist")
        .node_data()
        .meta
        .uuid
        .0
        .to_string();
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .expect("Processor parameter should exist")
            .node_data()
            .meta
            .decl_id
        .0,
        format!("surface/{source_uuid}")
    );

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}bool"),
        label: Some("Enabled".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Formula update should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Processor should receive the Formula hierarchy event");
    engine
        .apply_edits()
        .expect("Processor property reconciliation should stabilize");
    let snapshot = engine.process_tree_snapshot();
    assert!(
        snapshot.child_ids(instance_properties).into_iter().any(|child| {
            engine.nodes.get(child).is_some_and(|node| {
                node.engine_param_snapshot().is_some()
                    && node.node_data().meta.label == "Enabled"
            })
        }),
        "Processor should follow Formula property additions"
    );
}

#[test]
fn processor_mirrors_formula_property_folders_and_nested_properties() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    let properties = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        .expect("Formula Properties should exist");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: PROPERTY_FOLDER_NODE_TYPE.to_owned(),
        label: Some("Group".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Folder creation should succeed: {ack:?}");

    let formula_folder = engine
        .process_tree_snapshot()
        .child_ids(properties)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.get_type() == PROPERTY_FOLDER_NODE_TYPE)
        })
        .expect("Folder should exist in formula properties");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula_folder,
        node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
        label: Some("Speed".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Nested property creation should succeed: {ack:?}");

    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");

    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");

    let snapshot = engine.process_tree_snapshot();
    let instance_properties = snapshot
        .find_child_by_decl_id(processor_id, PROPERTIES_DECL_ID)
        .expect("Processor should instantiate Formula Properties");

    let instance_folder = snapshot
        .child_ids(instance_properties)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.get_type() == StateProcessorFolder::NODE_TYPE)
        })
        .expect("Processor should mirror formula folder");

    assert_eq!(
        engine
            .nodes
            .get(instance_folder)
            .map(|n| n.node_data().meta.label.as_str()),
        Some("Group"),
        "Mirrored folder should preserve formula folder label"
    );

    let nested = snapshot
        .child_ids(instance_folder)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.engine_param_snapshot().is_some())
        })
        .expect("Processor folder should contain the nested property");

    assert_eq!(
        engine
            .nodes
            .get(nested)
            .map(|n| n.node_data().meta.label.as_str()),
        Some("Speed"),
        "Nested property should be mirrored inside processor folder"
    );

    // verify dynamic reconciliation: add a second property to the formula folder
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula_folder,
        node_type: format!("{PROPERTY_CREATE_PREFIX}bool"),
        label: Some("Active".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Dynamic nested property addition should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Processor should receive hierarchy event");
    engine
        .apply_edits()
        .expect("Reconciliation should stabilize");

    let snapshot = engine.process_tree_snapshot();
    let instance_folder = snapshot
        .child_ids(instance_properties)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.get_type() == StateProcessorFolder::NODE_TYPE)
        })
        .expect("Processor folder should still exist after reconciliation");

    assert!(
        snapshot.child_ids(instance_folder).into_iter().any(|child| {
            engine
                .nodes
                .get(child)
                .is_some_and(|n| n.node_data().meta.label == "Active")
        }),
        "Dynamically added nested property should appear in processor folder"
    );
}
