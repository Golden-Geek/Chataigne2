use std::collections::HashMap;

use chataigne_state_machine::{ANodeOutputPreviewSample, ProcessorId};
use golden_alchemist::{
    ANodeId, ExecNodeId, FormulaId, OutputPreviewStatus, RuntimeValue, SocketId,
};
use golden_core::{
    engine::EngineTime,
    node::{DeclId, Folder, Node},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ExecutionPhase, ProcessCtx},
    ui_sync::UiEditIntent,
};

use super::{
    merge_output_preview_snapshot,
    next_input_value_condition_valid_state,
    processor_formula_from_snapshot, processor_formula_source_ref,
    processor_override_value, STATE_ITEM_KIND, StateMachineManager,
};
use crate::app::state_machine_nodes_processor::{FormulaCatalog, FormulaSourceRef};

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
fn input_value_condition_toggle_uses_inner_invalid_to_valid_edge() {
    assert!(next_input_value_condition_valid_state(false, false, false, true));
    assert!(!next_input_value_condition_valid_state(false, true, true, false));

    assert!(next_input_value_condition_valid_state(true, false, false, true));
    assert!(next_input_value_condition_valid_state(true, true, true, true));
    assert!(next_input_value_condition_valid_state(true, true, true, false));
    assert!(!next_input_value_condition_valid_state(true, true, false, true));
}

fn preview_sample(
    formula_id: FormulaId,
    processor_id: ProcessorId,
    author_node_id: ANodeId,
    socket: &str,
    value: RuntimeValue,
    logical_tick: u64,
) -> ANodeOutputPreviewSample {
    ANodeOutputPreviewSample {
        formula_id,
        processor_id: Some(processor_id),
        context_key: None,
        author_node_id,
        exec_node: ExecNodeId::new(0),
        output_socket: SocketId::new(socket),
        value_type: value.value_type(),
        value,
        logical_tick,
        status: OutputPreviewStatus::Live,
    }
}

#[test]
fn output_preview_snapshot_retains_latest_values_absent_from_next_delta() {
    let formula_id = FormulaId::new("formula");
    let processor_id = ProcessorId::new();
    let condition_node = ANodeId::new();
    let value_node = ANodeId::new();
    let mut snapshot = HashMap::new();

    let condition_valid = preview_sample(
        formula_id.clone(),
        processor_id,
        condition_node,
        "valid",
        RuntimeValue::Bool(true),
        10,
    );
    let first_frame = merge_output_preview_snapshot(&mut snapshot, vec![condition_valid.clone()]);
    assert_eq!(first_frame, vec![condition_valid.clone()]);

    let value_update = preview_sample(
        formula_id,
        processor_id,
        value_node,
        "value",
        RuntimeValue::Float(0.75),
        11,
    );
    let second_frame = merge_output_preview_snapshot(&mut snapshot, vec![value_update.clone()]);

    assert_eq!(second_frame.len(), 2);
    assert!(second_frame.contains(&condition_valid));
    assert!(second_frame.contains(&value_update));
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
fn processor_formula_resolver_reads_project_source_key() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(crate::app::FormulaLibrary::new().into(), None);
    engine
        .apply_edits()
        .expect("formula library should attach");
    let library_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("formula library should exist");
    let mut formula = crate::app::AlchemistFormulaDefinition::new();
    formula.node_data_mut().meta.label = "Project Formula".to_owned();
    let formula_uuid = formula.node_data().meta.uuid;
    engine.add_user_item(formula.into(), Some(library_id));
    engine
        .apply_edits()
        .expect("project formula should attach");

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
        node_type: format!("state_processor:project:{}", formula_uuid.0),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(ack.success, "project formula processor should attach: {ack:?}");
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
        FormulaSourceRef::ProjectNode(reference) if reference.uuid() == formula_uuid
    ));

    let catalog = FormulaCatalog::from_snapshot(&snapshot);
    let formula_id = snapshot
        .node_id_by_uuid(formula_uuid)
        .expect("project formula should exist");
    let formula_map = HashMap::from([(
        formula_uuid,
        crate::app::state_machine_nodes_formula::formula_from_snapshot(&snapshot, formula_id)
            .expect("project formula should materialize"),
    )]);
    let (formula_node, formula, formula_ui, formula_source_key) =
        processor_formula_from_snapshot(&snapshot, processor_id, &formula_map, &catalog)
            .expect("project formula should resolve from catalog");
    assert!(formula_node.is_some());
    assert_eq!(formula.label, "Project Formula");
    assert_eq!(
        formula_source_key,
        format!("state_processor:project:{}", formula_uuid.0)
    );
    assert_eq!(
        formula_ui.source_kind,
        chataigne_state_machine::ProcessorFormulaSourceKind::Project
    );
    assert!(!formula_ui.open_readonly_from_processor);
    assert!(!formula_ui.can_duplicate_to_library);
}
