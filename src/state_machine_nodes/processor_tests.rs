use golden_core::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeId, NodeMetaPatch, PresentationHint},
    parameter::{ParamValue, Parameter, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    ui_sync::UiEditIntent,
};

use crate::app::{
    AlchemistFormulaDefinition, AppEngine, AppNode, ConditionManager, FilterChainManager,
    FormulaLibrary, InputsManager, OutputsManager, StateMachineManager,
};

use super::{
    FormulaCatalog, FormulaSourceRef,
    PROCESSOR_FOLDER_ITEM_KIND, PROCESSOR_FOLDER_NODE_TYPE,
    PROCESSOR_FORMULA_SOURCE_DECL_ID, PROCESSOR_ITEM_KIND,
    PROCESSOR_MANAGED_REGIONS_DECL_ID, StateProcessor,
    StateProcessorFolder, StateProcessorManagedRegion,
    StateProcessorManagedRegions, StateProcessorManager,
    processor_managed_region_decl_id,
};
use crate::app::state_machine_nodes_formula::{
    AlchemistProperty, ANODE_CREATE_PREFIX, ANODE_NODE_TYPE,
    FORMULA_MANAGED_REGIONS_JSON_DECL_ID, PROPERTIES_DECL_ID, PROPERTY_CREATE_PREFIX,
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
    let (mut engine, _, formula_uuid) = engine_with_formula();
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

    let labels = formula_items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(labels, vec!["Custom Formula"]);
    assert_eq!(
        formula_items[0].node_type,
        format!("state_processor:project:{}", formula_uuid.0)
    );
    assert!(formula_items[0].menu_path.is_empty());
}

#[test]
fn formula_library_lists_formula_creation_items_only() {
    let (engine, _, _) = engine_with_formula();
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(_, node)| node)
        .expect("formula library should exist");
    let labels = library
        .user_creatable_items()
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();

    assert!(labels.iter().any(|label| label == "Formula"));
    assert!(labels.iter().any(|label| label == "Folder"));
}

#[test]
fn legacy_builtin_processor_source_is_not_creatable() {
    let manager = StateProcessorManager::new();

    let source = "state_processor:builtin:example.formula@1";
    FormulaSourceRef::parse_processor_create_type(source)
        .expect_err("legacy built-in sources should not parse");
    let processor = manager.create_user_item(source);

    assert!(
        processor.is_none(),
        "legacy built-in processor sources must fail at the creation boundary"
    );
}

#[test]
fn processor_created_from_typed_project_item_keeps_source_and_reference() {
    let (_, _, formula_uuid) = engine_with_formula();
    let manager = StateProcessorManager::new();
    let processor = manager
        .create_user_item(&format!("state_processor:project:{}", formula_uuid.0))
        .expect("typed project processor should be creatable");
    let processor = processor
        .as_any()
        .downcast_ref::<StateProcessor>()
        .expect("created item should be a state processor");

    assert!(matches!(
        processor.formula_source.to_source_ref(),
        Ok(Some(FormulaSourceRef::ProjectNode(reference))) if reference.uuid() == formula_uuid
    ));
    assert_eq!(processor.formula.get_ref().uuid(), formula_uuid);
}

#[test]
fn project_formula_processor_instantiates_managed_region_folders() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    seed_formula_managed_regions(&mut engine, formula, value_pipeline_regions_json());
    let processor_id = attach_processor_referencing(&mut engine, formula_uuid);
    let snapshot = engine.process_tree_snapshot();
    let source_key = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .expect("Processor should expose its formula source key");

    assert_eq!(
        snapshot
            .node(source_key)
            .and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Str(format!(
            "state_processor:project:{}",
            formula_uuid.0
        )))
    );

    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own a Managed Regions root");
    assert_eq!(
        engine
            .nodes
            .get(regions_root)
            .expect("Managed Regions root should exist")
            .get_type(),
        StateProcessorManagedRegions::NODE_TYPE
    );

    let actual = snapshot
        .child_ids(regions_root)
        .into_iter()
        .map(|child| {
            let node = engine.nodes.get(child).expect("region should exist");
            (
                node.get_type().to_owned(),
                node.node_data().meta.decl_id.0.clone(),
                node.node_data().meta.label.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("inputs"),
                "Inputs".to_owned()
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("filters"),
                "Filters".to_owned()
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("outputs"),
                "Outputs".to_owned()
            )
        ]
    );
}

#[test]
fn builtin_action_processor_created_inside_state_exposes_managers() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    for formula in FormulaCatalog::default_builtin_formula_trees()
        .expect("built-in formulas should load")
    {
        library_tree.push_child(formula);
    }
    engine.edits.push(Edit::AddNodeTree {
        tree: library_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.add_node(StateMachineManager::new().into(), None);
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("default formula and state machine roots should attach");
    }
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("state machine manager should exist");
    let action_formula = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
                && node.node_data().meta.label == "Action"
        })
        .map(|(id, node)| (id, node.node_data().meta.uuid))
        .expect("built-in Action formula should be materialized in the formula library");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: manager_id,
        node_type: crate::app::StateMachineState::NODE_TYPE.to_owned(),
        label: Some("State".to_owned()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "state creation should succeed: {ack:?}");
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("state children should materialize");
    }

    let snapshot = engine.process_tree_snapshot();
    let state_id = snapshot
        .child_ids(manager_id)
        .into_iter()
        .find(|state| {
            snapshot
                .node(*state)
                .is_some_and(|node| node.node_type == crate::app::StateMachineState::NODE_TYPE)
        })
        .expect("state should exist");
    let processor_manager_id = snapshot
        .find_child_by_decl_id(state_id, "processors")
        .expect("state should have a processor manager");

    let action_item = FormulaCatalog::from_snapshot(&snapshot)
        .processor_palette_items()
        .into_iter()
        .find(|item| item.label == "Action")
        .expect("built-in Action formula should be exposed as a processor item");
    assert_eq!(
        action_item.node_type,
        FormulaSourceRef::project_uuid(action_formula.1).processor_create_type()
    );
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: processor_manager_id,
        node_type: action_item.node_type.clone(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        ack.success,
        "built-in Action processor creation should succeed: {ack:?}"
    );
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("Action processor surface should materialize");
    }

    let snapshot = engine.process_tree_snapshot();
    let processor_id = snapshot
        .child_ids(processor_manager_id)
        .into_iter()
        .find(|processor| {
            snapshot
                .node(*processor)
                .is_some_and(|node| node.node_type == StateProcessor::NODE_TYPE)
        })
        .expect("processor should exist");
    let source_key = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .expect("processor should expose its formula source key");
    assert_eq!(
        snapshot
            .node(source_key)
            .and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Str(action_item.node_type))
    );
    assert_eq!(snapshot.node_id_by_uuid(action_formula.1), Some(action_formula.0));
    let exposed_managers = snapshot
        .child_ids(processor_id)
        .into_iter()
        .filter_map(|child| {
            let node = engine.nodes.get(child)?;
            matches!(
                node.get_type(),
                ConditionManager::NODE_TYPE | OutputsManager::NODE_TYPE
            )
            .then(|| {
                (
                    node.get_type().to_owned(),
                    node.node_data().meta.label.clone(),
                )
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        exposed_managers,
        vec![
            (
                ConditionManager::NODE_TYPE.to_owned(),
                "Conditions".to_owned(),
            ),
            (OutputsManager::NODE_TYPE.to_owned(), "On True".to_owned()),
            (OutputsManager::NODE_TYPE.to_owned(), "On False".to_owned()),
        ]
    );

    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own a Managed Regions root");

    let actual = snapshot
        .child_ids(regions_root)
        .into_iter()
        .map(|child| {
            let node = engine.nodes.get(child).expect("region should exist");
            (
                node.get_type().to_owned(),
                node.node_data().meta.decl_id.0.clone(),
                node.node_data().meta.label.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("conditions"),
                "Conditions".to_owned(),
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("on_true"),
                "On True".to_owned(),
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("on_false"),
                "On False".to_owned(),
            ),
        ]
    );
}

#[test]
fn managed_region_palette_accepts_only_matching_anode_roles() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    seed_formula_managed_regions(&mut engine, formula, value_pipeline_regions_json());
    let processor_id = attach_processor_referencing(&mut engine, formula_uuid);
    let snapshot = engine.process_tree_snapshot();
    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own Managed Regions");
    let filters = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("filters"),
        )
        .expect("formula should expose a Filters region");
    let inputs = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("inputs"),
        )
        .expect("formula should expose an Inputs region");
    let outputs = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("outputs"),
        )
        .expect("formula should expose an Outputs region");
    let condition_gate_type = format!("{ANODE_CREATE_PREFIX}condition_gate");

    let filter_items = engine
        .nodes
        .get(filters)
        .expect("Filters region should exist")
        .user_creatable_items();
    let input_items = engine
        .nodes
        .get(inputs)
        .expect("Inputs region should exist")
        .user_creatable_items();
    let output_items = engine
        .nodes
        .get(outputs)
        .expect("Outputs region should exist")
        .user_creatable_items();

    assert!(filter_items
        .iter()
        .any(|item| item.node_type == condition_gate_type));
    assert!(input_items.is_empty());
    assert!(output_items.is_empty());
    assert!(!input_items
        .iter()
        .any(|item| item.node_type == condition_gate_type));

    let rejected = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: inputs,
        node_type: condition_gate_type.clone(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        !rejected.success,
        "Inputs region should reject filter-only ANodes"
    );

    let accepted = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: filters,
        node_type: condition_gate_type,
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        accepted.success,
        "Filters region should accept ConditionGate: {accepted:?}"
    );
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.child_ids(filters).into_iter().any(|child| {
        engine
            .nodes
            .get(child)
            .is_some_and(|node| node.get_type() == ANODE_NODE_TYPE)
    }));

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
    processor.set_formula_source(FormulaSourceRef::project_uuid(formula_uuid));
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
        ("filter", "Filters"),
        ("input", "Inputs"),
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
    processor.set_formula_source(FormulaSourceRef::project_uuid(formula_uuid));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let snapshot = engine.process_tree_snapshot();
    // Properties are mirrored flat at the processor's top level (no Properties folder).
    let instance_children = snapshot.child_ids(processor_id);
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
        (FilterChainManager::NODE_TYPE, "Filters"),
        (InputsManager::NODE_TYPE, "Inputs"),
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
        snapshot.child_ids(processor_id).into_iter().any(|child| {
            engine.nodes.get(child).is_some_and(|node| {
                node.engine_param_snapshot().is_some()
                    && node.node_data().meta.label == "Enabled"
            })
        }),
        "Processor should follow Formula property additions"
    );
}

#[test]
fn processor_mirrors_formula_property_order() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    let properties = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        .expect("Formula Properties should exist");

    for label in ["Amount", "Enabled"] {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: properties,
            node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
            label: Some(label.into()),
            initial_params: Vec::new(),
        });
        assert!(
            ack.success,
            "Formula property creation should succeed: {ack:?}"
        );
    }

    let formula_children = engine.process_tree_snapshot().child_ids(properties);
    let amount = formula_children
        .iter()
        .copied()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|node| node.node_data().meta.label == "Amount")
        })
        .expect("Amount property should exist");
    let enabled = formula_children
        .iter()
        .copied()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|node| node.node_data().meta.label == "Enabled")
        })
        .expect("Enabled property should exist");

    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");

    let mut processor = StateProcessor::new();
    processor.set_formula_source(FormulaSourceRef::project_uuid(formula_uuid));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    // Properties are mirrored flat at the processor's top level, so only the
    // `surface/`-prefixed property mirrors count (not the formula reference etc.).
    let processor_property_labels = |engine: &AppEngine| {
        engine
            .process_tree_snapshot()
            .child_ids(processor_id)
            .into_iter()
            .filter_map(|child| {
                engine.nodes.get(child).and_then(|node| {
                    let data = node.node_data();
                    (node.engine_param_snapshot().is_some()
                        && data.meta.decl_id.0.starts_with("surface/"))
                    .then(|| data.meta.label.clone())
                })
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        processor_property_labels(&engine),
        vec!["Amount".to_owned(), "Enabled".to_owned()]
    );

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: enabled,
        new_parent: properties,
        new_prev_sibling: None,
    });
    assert!(
        ack.success,
        "Formula property reorder should succeed: {ack:?}"
    );
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Processor should receive the Formula hierarchy event");
    engine
        .apply_edits()
        .expect("Processor property order reconciliation should stabilize");

    assert_eq!(
        engine
            .process_tree_snapshot()
            .child_ids(properties)
            .into_iter()
            .filter_map(|child| engine
                .nodes
                .get(child)
                .map(|node| node.node_data().meta.label.clone()))
            .collect::<Vec<_>>(),
        vec!["Enabled".to_owned(), "Amount".to_owned()]
    );
    assert_eq!(
        processor_property_labels(&engine),
        vec!["Enabled".to_owned(), "Amount".to_owned()]
    );
    assert_ne!(amount, enabled);
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
    processor.set_formula_source(FormulaSourceRef::project_uuid(formula_uuid));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");

    let snapshot = engine.process_tree_snapshot();
    // Property surfaces (including mirrored folders) sit flat under the processor.
    let instance_folder = snapshot
        .child_ids(processor_id)
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
        .child_ids(processor_id)
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

fn node_has_warning_id(engine: &AppEngine, node: NodeId, warning_id: &str) -> bool {
    engine
        .process_tree_snapshot()
        .node(node)
        .is_some_and(|snapshot| {
            snapshot
                .presentation
                .warnings
                .iter()
                .any(|warning| warning.id == warning_id)
    })
}

fn seed_formula_managed_regions(engine: &mut AppEngine, formula: NodeId, regions_json: &str) {
    let param = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
        .expect("formula should expose managed region metadata");
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: param,
        value: ParamValue::Str(regions_json.to_owned()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "managed region metadata should be set: {ack:?}");
    engine
        .apply_edits()
        .expect("managed region metadata should apply");
}

fn value_pipeline_regions_json() -> &'static str {
    r#"[
        {
            "id": "inputs",
            "kind": "input_set",
            "label": "Inputs",
            "input_socket": null,
            "output_socket": null,
            "accepted_roles": ["Input"]
        },
        {
            "id": "filters",
            "kind": "filter_pipeline",
            "label": "Filters",
            "input_socket": null,
            "output_socket": null,
            "accepted_roles": ["Filter"]
        },
        {
            "id": "outputs",
            "kind": "output_set",
            "label": "Outputs",
            "input_socket": null,
            "output_socket": null,
            "accepted_roles": ["Output"]
        }
    ]"#
}

fn attach_processor_referencing(
    engine: &mut AppEngine,
    formula_uuid: golden_core::node::NodeUuid,
) -> NodeId {
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.set_formula_source(FormulaSourceRef::project_uuid(formula_uuid));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist")
}

#[test]
fn processor_with_valid_formula_has_no_warning() {
    let (mut engine, _, formula_uuid) = engine_with_formula();
    let processor_id = attach_processor_referencing(&mut engine, formula_uuid);
    assert!(
        !node_has_warning_id(&engine, processor_id, super::PROCESSOR_FORMULA_WARNING_ID),
        "Processor with a valid formula reference should not warn"
    );
}

#[test]
fn processor_with_missing_formula_reference_sets_warning() {
    let (mut engine, _, _) = engine_with_formula();
    let missing_uuid = golden_core::node::NodeUuid(uuid::Uuid::new_v4());
    let processor_id = attach_processor_referencing(&mut engine, missing_uuid);
    assert!(
        node_has_warning_id(&engine, processor_id, super::PROCESSOR_FORMULA_WARNING_ID),
        "Processor referencing a missing formula should expose a warning"
    );
}

#[test]
fn processor_mirrors_formula_icon() {
    let (mut engine, formula_id, formula_uuid) = engine_with_formula();
    let processor_id = attach_processor_referencing(&mut engine, formula_uuid);

    assert_eq!(
        engine
            .nodes
            .get(processor_id)
            .expect("Processor should exist")
            .node_data()
            .meta
            .presentation
            .icon,
        None,
        "Processor should have no icon before its formula has one"
    );

    let icon = "data:image/svg+xml;base64,PHN2Zy8+";
    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: formula_id,
        patch: NodeMetaPatch {
            presentation: Some(PresentationHint {
                icon: Some(icon.to_owned()),
                ..PresentationHint::default()
            }),
            ..NodeMetaPatch::default()
        },
    });
    assert!(ack.success, "Formula icon patch should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Processor should receive the formula icon change");
    engine
        .apply_edits()
        .expect("Processor icon mirroring should stabilize");

    assert_eq!(
        engine
            .nodes
            .get(processor_id)
            .expect("Processor should exist")
            .node_data()
            .meta
            .presentation
            .icon
            .as_deref(),
        Some(icon),
        "Processor should mirror its formula's icon"
    );
}
