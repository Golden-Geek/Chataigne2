use std::{collections::HashMap, sync::Arc, time::Duration};

use chataigne_state_machine::{
    ANodeOutputPreviewSample, DefaultProcessorContextProvider, Processor, ProcessorDebugCapture,
    ProcessorId, ProcessorLifecycleEvent, ProcessorLifecyclePolicy, ProcessorRuntime, ValueSet,
};
use golden_alchemist::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraph, CompileCtx,
    EvaluationCtx, ExecNodeId, FormulaContextContract, FormulaId, FormulaPropertySchema,
    FormulaSurface, InputSocketRef, OutputPreviewStatus, OutputSocketRef, RuntimeInputSnapshot,
    RuntimeOutput, RuntimeRegistries, RuntimeValue, SocketId, TriggerValue, ValueTypeRegistry,
    primitive_node_registry,
};
use golden_core::{
    app::ProjectNode,
    engine::EngineTime,
    node::{DeclId, Folder, Node},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ExecutionPhase, ProcessCtx},
    ui_sync::UiEditIntent,
};

use super::{
    compile_processor_runtime_for_cache_rebuild, condition_manager_edge_previous,
    condition_manager_value_set, merge_output_preview_snapshot,
    next_input_value_condition_validity, next_input_value_condition_valid_state,
    output_preview_signature, processor_formula_from_snapshot, processor_formula_source_ref,
    processor_override_value, processor_should_evaluate, set_output_target_param,
    runtime_invalidation_for_node, should_emit_runtime_log, RuntimeInvalidation, RuntimeLogKey,
    StateMachineManager, PROCESSOR_MANAGER_DECL_ID, STATE_ITEM_KIND,
};
use crate::app::state_machine_nodes_processor::{
    FormulaCatalog, FormulaSourceRef, PROCESSOR_FORMULA_SOURCE_DECL_ID,
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
fn output_target_param_write_skips_unchanged_non_trigger_values() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(
        Parameter::new(
            "Target",
            ParamValue::Float(1.0),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    engine
        .apply_edits()
        .expect("target parameter should attach");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("target parameter should exist");
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.set_tree_snapshot(snapshot.clone());

    assert!(!set_output_target_param(
        &mut ctx,
        snapshot.as_ref(),
        target,
        ParamValue::Float(1.0)
    ));
    assert!(ctx.edits.pending.is_empty());
    assert!(set_output_target_param(
        &mut ctx,
        snapshot.as_ref(),
        target,
        ParamValue::Trigger()
    ));
    assert_eq!(ctx.edits.pending.len(), 1);
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

#[test]
fn transient_input_value_condition_pulses_then_settles_invalid() {
    let fired = next_input_value_condition_validity(false, false, false, true, true);
    assert!(fired.current);
    assert!(!fired.settled);

    let idle = next_input_value_condition_validity(false, false, false, false, true);
    assert!(!idle.current);
    assert!(!idle.settled);
}

#[test]
fn condition_manager_value_set_only_fires_transition_edges() {
    let mut next_trigger_edge_id = 0;
    let transition = condition_manager_value_set(7, true, Some(false), &mut next_trigger_edge_id);
    assert!(value_set_trigger_fired(&transition, "on_true"));
    assert!(!value_set_trigger_fired(&transition, "on_false"));

    let steady = condition_manager_value_set(7, true, Some(true), &mut next_trigger_edge_id);
    assert!(!value_set_trigger_fired(&steady, "on_true"));
    assert!(!value_set_trigger_fired(&steady, "on_false"));
}

#[test]
fn initial_condition_observation_without_dirty_source_is_not_an_edge() {
    assert_eq!(condition_manager_edge_previous(None, true, false), Some(true));
    assert_eq!(condition_manager_edge_previous(None, false, false), Some(false));
    assert_eq!(condition_manager_edge_previous(None, true, true), None);
}

#[test]
fn processor_evaluation_requires_runtime_or_signal_reason() {
    assert!(!processor_should_evaluate(false, false, false, false));
    assert!(processor_should_evaluate(true, false, false, false));
    assert!(processor_should_evaluate(false, true, false, false));
    assert!(processor_should_evaluate(false, false, true, false));
    assert!(processor_should_evaluate(false, false, false, true));
}

#[test]
fn processor_root_invalidation_rebuilds_topology_but_descendants_are_local() {
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
    engine
        .apply_edits()
        .expect("state should attach to the manager");
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
        .find_child_by_decl_id(state_id, PROCESSOR_MANAGER_DECL_ID)
        .expect("state should have a processor manager");

    engine.add_user_item(
        crate::app::StateProcessor::new().into(),
        Some(processor_manager_id),
    );
    engine
        .apply_edits()
        .expect("processor should attach to the processor manager");
    let snapshot = engine.process_tree_snapshot();
    let processor_id = snapshot
        .child_ids(processor_manager_id)
        .into_iter()
        .find(|processor| {
            snapshot
                .node(*processor)
                .is_some_and(|node| node.node_type == crate::app::StateProcessor::NODE_TYPE)
        })
        .expect("processor should exist");
    let formula_source_key = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .expect("processor should have a formula source key child");

    assert_eq!(
        runtime_invalidation_for_node(&snapshot, manager_id, processor_id),
        RuntimeInvalidation::Topology
    );
    assert_eq!(
        runtime_invalidation_for_node(&snapshot, manager_id, formula_source_key),
        RuntimeInvalidation::Processor(processor_id)
    );
}

#[test]
fn duplicated_processor_marks_manager_topology_dirty_after_queued_structure_dispatch() {
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
    engine
        .apply_edits()
        .expect("state should attach to the manager");
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
        .find_child_by_decl_id(state_id, PROCESSOR_MANAGER_DECL_ID)
        .expect("state should have a processor manager");

    engine.add_user_item(
        crate::app::StateProcessor::new().into(),
        Some(processor_manager_id),
    );
    engine
        .apply_edits()
        .expect("processor should attach to the processor manager");
    run_manager_runtime_tick(&mut engine);
    assert!(
        !manager_topology_dirty(&engine, manager_id),
        "initial processor topology should be rebuilt before duplication",
    );

    let snapshot = engine.process_tree_snapshot();
    let processor_id = snapshot
        .child_ids(processor_manager_id)
        .into_iter()
        .find(|processor| {
            snapshot
                .node(*processor)
                .is_some_and(|node| node.node_type == crate::app::StateProcessor::NODE_TYPE)
        })
        .expect("processor should exist");
    engine
        .duplicate_subtree_with(
            processor_id,
            processor_manager_id,
            Some(processor_id),
            Some("Processor Copy".to_owned()),
            |node| node.project_encode_data(),
            <crate::app::AppNode as ProjectNode>::project_decode_node,
        )
        .expect("processor duplicate should succeed");

    assert!(
        !manager_topology_dirty(&engine, manager_id),
        "duplicating a processor should not synchronously rebuild state-machine topology",
    );

    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EngineTick)
        .expect("queued duplicate structure events should dispatch");

    assert!(
        manager_topology_dirty(&engine, manager_id),
        "queued duplicate structure events must wake the state-machine runtime before evaluation",
    );
}

#[test]
fn cache_rebuild_compile_preserves_stateful_trigger_memory() {
    let formula = stateful_trigger_formula();
    let mut processor = Processor::from_formula("Processor", &formula);
    processor.lifecycle = ProcessorLifecyclePolicy::AlwaysActive;
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    assert!(compile_processor_runtime_for_cache_rebuild(
        &mut runtime,
        &processor,
        &formula,
        &compile_ctx,
    ));
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::ProjectStart);

    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let provider = DefaultProcessorContextProvider;
    let capture = ProcessorDebugCapture::All { history_len: 64 };
    let first_ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let first = runtime.evaluate_processor_with_context_provider_and_send_capture(
        &processor,
        &first_ctx,
        &provider,
        &capture,
    );
    assert_eq!(first.len(), 1);
    assert!(runtime_output_trigger_fired(&first[0].output));

    assert!(compile_processor_runtime_for_cache_rebuild(
        &mut runtime,
        &processor,
        &formula,
        &compile_ctx,
    ));
    let second_ctx = EvaluationCtx {
        logical_tick: 2,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let second = runtime.evaluate_processor_with_context_provider_and_send_capture(
        &processor,
        &second_ctx,
        &provider,
        &capture,
    );

    assert_eq!(second.len(), 1);
    assert!(!runtime_output_trigger_fired(&second[0].output));
    assert_eq!(runtime.lanes.memory_count(), 1);
}

#[test]
fn manager_shared_compile_cache_compiles_formula_once() {
    let formula = stateful_trigger_formula();
    let mut manager = StateMachineManager::new();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };

    let first = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should compile");
    let second = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should come from cache");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(manager.runtime_perf_stats().formula_compiles, 1);
}

fn run_manager_runtime_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("state-machine inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("state-machine runtime tick should run");
    engine
        .apply_edits()
        .expect("state-machine runtime edits should apply");
}

fn manager_topology_dirty(
    engine: &crate::app::AppEngine,
    manager_id: golden_core::node::NodeId,
) -> bool {
    let crate::app::AppNode::StateMachineManager(manager) = engine
        .nodes
        .get(manager_id)
        .expect("state machine manager should exist")
    else {
        panic!("expected StateMachineManager node");
    };
    manager.runtime_topology_dirty()
}

fn value_set_trigger_fired(value_set: &ValueSet, key: &str) -> bool {
    value_set.entries.iter().any(|entry| {
        entry.key.as_str() == key
            && matches!(&entry.value, RuntimeValue::Trigger(trigger) if trigger.fired)
    })
}

fn runtime_output_trigger_fired(output: &RuntimeOutput) -> bool {
    output
        .debug_samples
        .iter()
        .any(|sample| matches!(&sample.value, RuntimeValue::Trigger(trigger) if trigger.fired))
}

fn stateful_trigger_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Bool(true));
    let source = graph.add_node(constant).unwrap();
    let edge = graph
        .add_node(ANodeInstance::new(
            ANodeTypeId::new("trigger_on_off"),
            "Trigger On/Off",
        ))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
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
fn output_preview_signature_is_order_independent_and_trigger_sensitive() {
    let formula_id = FormulaId::new("formula");
    let processor_id = ProcessorId::new();
    let value_node = ANodeId::new();
    let trigger_node = ANodeId::new();
    let value = preview_sample(
        formula_id.clone(),
        processor_id,
        value_node,
        "value",
        RuntimeValue::Float(0.75),
        10,
    );
    let trigger = preview_sample(
        formula_id.clone(),
        processor_id,
        trigger_node,
        "trigger",
        RuntimeValue::Trigger(TriggerValue::fired(7, 10)),
        10,
    );
    let changed_trigger = preview_sample(
        formula_id,
        processor_id,
        trigger_node,
        "trigger",
        RuntimeValue::Trigger(TriggerValue::fired(8, 10)),
        10,
    );

    let signature = output_preview_signature(&[value.clone(), trigger.clone()]);
    assert_eq!(
        signature,
        output_preview_signature(&[trigger, value.clone()])
    );
    assert_ne!(
        signature,
        output_preview_signature(&[value, changed_trigger])
    );
}

#[test]
fn runtime_log_dedupe_uses_typed_processor_and_kind_key() {
    let manager = StateMachineManager::new();
    let processor_node = manager.id();
    let mut last_values = HashMap::new();

    assert!(should_emit_runtime_log(
        &mut last_values,
        10,
        RuntimeLogKey::processor_compile(processor_node),
        "same diagnostic",
    ));
    assert!(!should_emit_runtime_log(
        &mut last_values,
        40,
        RuntimeLogKey::processor_compile(processor_node),
        "same diagnostic",
    ));
    assert!(should_emit_runtime_log(
        &mut last_values,
        40,
        RuntimeLogKey::processor_runtime(processor_node),
        "same diagnostic",
    ));
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
