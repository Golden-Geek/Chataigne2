use std::collections::HashMap;

use chataigne_state_machine::ProcessorFormulaSourceKind;
use golden_core::{
    engine::EngineTime,
    node::{DeclId, Folder, Node},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ExecutionPhase, ProcessCtx},
    ui_sync::UiEditIntent,
};

use super::{
    processor_formula_from_snapshot, processor_formula_source_ref,
    processor_override_value, STATE_ITEM_KIND, StateMachineManager,
};
use crate::app::state_machine_nodes_processor::{
    FormulaCatalog, FormulaSourceRef, BUILTIN_FORMULA_PACKAGE,
    BUILTIN_FORMULA_VERSION, BUILTIN_MAPPING_FORMULA_ID,
};

#[test]
fn state_machine_manager_is_fixed_and_creates_states() {
    let mut manager = StateMachineManager::new();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    manager.init(&mut ctx);

    assert!(!manager.node_data().meta.user_permissions.can_remove_and_duplicate);
    let items = manager.user_creatable_items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].node_type, crate::app::StateMachineState::NODE_TYPE);
    assert_eq!(items[0].item_kind, STATE_ITEM_KIND);
    assert_eq!(items[0].label, "State");

    let state = manager
        .create_user_item(crate::app::StateMachineState::NODE_TYPE)
        .expect("state machine manager should create state items");
    assert_eq!(state.get_type(), crate::app::StateMachineState::NODE_TYPE);
}

#[test]
fn states_do_not_accept_user_items() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineManager::new().into(), None);
    engine
        .apply_edits()
        .expect("state machine manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("state machine manager should exist");

    engine.add_user_item(crate::app::StateMachineState::new().into(), Some(manager_id));
    engine.add_user_item(crate::app::StateMachineState::new().into(), Some(manager_id));
    engine
        .apply_edits()
        .expect("states should attach to the manager");

    let states = engine.process_tree_snapshot().child_ids(manager_id);
    assert_eq!(states.len(), 2);

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: states[1],
        new_parent: states[0],
        new_prev_sibling: None,
    });
    assert!(!ack.success, "states must not accept nested user items");
}

#[test]
fn processor_override_value_reads_direct_parameter_nodes() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut parameter = Parameter::new(
        "Amount",
        ParamValue::Float(7.5),
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.decl_id = DeclId("surface/amount".to_owned());
    engine.add_node(parameter.into(), None);
    engine
        .apply_edits()
        .expect("override parameter should attach");
    let parameter_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.decl_id.0 == "surface/amount")
        .map(|(id, _)| id)
        .expect("override parameter should exist");
    let snapshot = engine.process_tree_snapshot();

    assert_eq!(
        processor_override_value(&snapshot, parameter_id),
        Some(&ParamValue::Float(7.5))
    );
}

#[test]
fn processor_formula_resolver_reads_builtin_source_key() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(crate::app::StateProcessorManager::new().into(), None);
    engine
        .apply_edits()
        .expect("processor manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: manager_id,
        node_type: "state_processor:builtin:chataigne.mapping@1".to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(ack.success, "builtin Mapping processor should attach: {ack:?}");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor should exist");
    let snapshot = engine.process_tree_snapshot();
    let source = processor_formula_source_ref(&snapshot, processor_id)
        .expect("processor source should resolve");

    assert!(matches!(
        &source,
        FormulaSourceRef::Builtin {
            package,
            formula_id,
            version,
        } if package.as_ref() == BUILTIN_FORMULA_PACKAGE
            && formula_id.as_ref() == BUILTIN_MAPPING_FORMULA_ID
            && *version == BUILTIN_FORMULA_VERSION
    ));

    let catalog = FormulaCatalog::from_snapshot(&snapshot);
    let (formula_node, formula, formula_ui, formula_source_key) =
        processor_formula_from_snapshot(&snapshot, processor_id, &HashMap::new(), &catalog)
            .expect("builtin formula should resolve from catalog");
    assert!(formula_node.is_none());
    assert_eq!(formula.label, "Mapping");
    assert_eq!(formula_source_key, "state_processor:builtin:chataigne.mapping@1");
    assert_eq!(formula_ui.source_kind, ProcessorFormulaSourceKind::Builtin);
    assert!(formula_ui.open_readonly_from_processor);
    assert!(formula_ui.can_duplicate_to_library);
}
