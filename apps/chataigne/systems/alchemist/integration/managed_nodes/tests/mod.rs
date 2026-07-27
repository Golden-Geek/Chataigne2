use golden_core::edit::Edit;
use golden_core::engine::EngineTime;
use golden_core::node::{Folder, Node, NodeId};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use golden_core::process_ctx::{ExecutionPhase, ProcessCtx};
use golden_core::ui_sync::UiEditIntent;

use super::{
    ConditionManager, ConsequencesManager, FilterChainManager, InputsManager, OutputGroup,
    OutputRuntimeCache, OutputRuntimeTarget, OutputSchedule, OutputsManager,
};
use crate::app::module_command::{
    self, ModuleCommandDeliveryPolicy, ModuleCommandInvocationId,
    ModuleCommandParamOverride,
};

#[test]
fn managed_nodes_have_correct_type_ids() {
    assert_eq!(ConditionManager::NODE_TYPE, "sm_condition_manager");
    assert_eq!(ConsequencesManager::NODE_TYPE, "sm_consequences_manager");
    assert_eq!(InputsManager::NODE_TYPE, "sm_inputs_manager");
    assert_eq!(FilterChainManager::NODE_TYPE, "sm_filter_chain_manager");
    assert_eq!(OutputsManager::NODE_TYPE, "sm_outputs_manager");
}

#[test]
fn managed_nodes_are_non_removable_by_default() {
    let cm = ConditionManager::new();
    let csq = ConsequencesManager::new();
    let inp = InputsManager::new();
    let fc = FilterChainManager::new();
    let out = OutputsManager::new();

    for (label, perm) in [
        ("ConditionManager", &cm.node_data().meta.user_permissions),
        ("ConsequencesManager", &csq.node_data().meta.user_permissions),
        ("InputsManager", &inp.node_data().meta.user_permissions),
        ("FilterChainManager", &fc.node_data().meta.user_permissions),
        ("OutputsManager", &out.node_data().meta.user_permissions),
    ] {
        assert!(!perm.can_remove_and_duplicate, "{label} should not be removable");
        assert!(!perm.can_edit_name, "{label} should not have editable name");
    }
}

#[test]
fn condition_manager_operator_defaults_to_all() {
    let cm = ConditionManager::new();
    assert_eq!(cm.operator.get_ref().as_str(), "all");
    assert_eq!(*cm.operator_count.get_ref(), 1.0);
    assert!(!*cm.valid.get_ref());
}

#[test]
fn condition_manager_accepts_all_operators() {
    let mut cm = ConditionManager::new();
    for op in ["all", "any", "none", "at_least", "exactly"] {
        cm.operator.apply_runtime_value(&ParamValue::Str(op.to_string()));
        assert_eq!(cm.operator.get_ref().as_str(), op);
    }
}

#[test]
fn condition_manager_operator_visibility_tracks_condition_items() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ConditionManager::new().into(), None);
    engine
        .apply_edits()
        .expect("condition manager should attach");
    let manager = node_by_type(&engine, ConditionManager::NODE_TYPE);

    assert_operator_visibility(&engine, manager, false);

    create_input_value_condition(&mut engine, manager);
    assert_operator_visibility(&engine, manager, false);

    create_input_value_condition(&mut engine, manager);
    assert_operator_visibility(&engine, manager, true);
}

#[test]
fn output_containers_use_periodic_updates_for_delayed_outputs() {
    assert_eq!(OutputsManager::new().execution_rule().update_rate, Some(60));
    assert_eq!(OutputGroup::new().execution_rule().update_rate, Some(60));
}

#[test]
fn output_controls_live_in_collapsed_advanced_folder() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");
    let manager = node_by_type(&engine, OutputsManager::NODE_TYPE);
    let snapshot = engine.process_tree_snapshot();
    let advanced = snapshot
        .find_child_by_decl_id(manager, "advanced")
        .expect("advanced folder should exist");

    assert!(
        snapshot
            .node(advanced)
            .expect("advanced folder should be in the snapshot")
            .presentation
            .collapsed,
        "output advanced folder should start collapsed"
    );
    assert!(snapshot.find_child_by_decl_id(manager, "delay").is_none());
    assert!(snapshot.find_child_by_decl_id(manager, "stagger").is_none());
    assert!(
        snapshot
            .find_child_by_decl_id(manager, "cancel_on_trigger")
            .is_none()
    );
    assert_output_control_visibility(&engine, manager, "delay", true);
    assert_output_control_visibility(&engine, manager, "stagger", false);
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", false);
}

#[test]
fn output_stagger_visibility_tracks_output_count() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");
    let manager = node_by_type(&engine, OutputsManager::NODE_TYPE);

    assert_output_control_visibility(&engine, manager, "stagger", false);

    create_generic_log_output(&mut engine, manager);
    assert_output_control_visibility(&engine, manager, "stagger", false);

    create_generic_log_output(&mut engine, manager);
    assert_output_control_visibility(&engine, manager, "stagger", true);
}

#[test]
fn output_cancel_visibility_tracks_active_timing() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");
    let manager = node_by_type(&engine, OutputsManager::NODE_TYPE);

    create_generic_log_output(&mut engine, manager);
    create_generic_log_output(&mut engine, manager);
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", false);

    let delay = output_control(&engine, manager, "delay");
    set_param(&mut engine, delay, ParamValue::Float(0.25));
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", true);

    set_param(&mut engine, delay, ParamValue::Float(0.0));
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", false);

    let stagger = output_control(&engine, manager, "stagger");
    set_param(&mut engine, stagger, ParamValue::Float(0.25));
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", true);
}

#[test]
fn output_schedule_preserves_invocation_through_delay_and_stagger() {
    let invocation_id = ModuleCommandInvocationId::new(NodeId(40), 7);
    let first = NodeId(101);
    let second = NodeId(102);
    let cache = OutputRuntimeCache {
        delay: 0.5,
        stagger: 0.5,
        outputs: vec![output_target(first, false), output_target(second, false)],
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(&mut ctx, &cache, Vec::new(), Some(invocation_id));
    assert_eq!(schedule.pending.len(), 2);
    assert!(ctx.edits.pending.is_empty());

    schedule.tick(&mut ctx, 0.5);
    assert_eq!(execute_invocation(&ctx, first), Some(invocation_id));
    ctx.edits.pending.clear();

    schedule.tick(&mut ctx, 0.5);
    assert_eq!(execute_invocation(&ctx, second), Some(invocation_id));
    assert!(schedule.pending.is_empty());
}

#[test]
fn immediate_log_output_rejects_unchanged_streams_before_emitting_events() {
    let emitter = NodeId(40);
    let first = ModuleCommandInvocationId::new(emitter, 1);
    let deferred = ModuleCommandInvocationId::new(emitter, 2);
    let command = NodeId(101);
    let cache = OutputRuntimeCache {
        outputs: vec![output_target(command, true)],
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);
    let original = vec![ModuleCommandParamOverride {
        param_id: NodeId(201),
        value: ParamValue::Str("original".to_owned()),
    }];

    schedule.on_trigger_cached(&mut ctx, &cache, original.clone(), Some(first));
    let execute = execute_request(&ctx, command);
    assert_eq!(execute.invocation_id, Some(first));
    assert_eq!(
        execute.delivery_policy,
        ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted
    );
    ctx.edits.pending.clear();

    schedule.on_trigger_cached(&mut ctx, &cache, original.clone(), Some(first));
    assert!(ctx.edits.pending.is_empty());
    schedule.on_trigger_cached(&mut ctx, &cache, original.clone(), Some(deferred));
    assert!(ctx.edits.pending.is_empty());

    ctx.time.tick = 2;
    schedule.on_trigger_cached(&mut ctx, &cache, original.clone(), Some(deferred));
    assert_eq!(execute_invocation(&ctx, command), Some(deferred));
    ctx.edits.pending.clear();

    ctx.time.tick = 31;
    let changed = vec![ModuleCommandParamOverride {
        param_id: NodeId(201),
        value: ParamValue::Str("changed".to_owned()),
    }];
    schedule.on_trigger_cached(&mut ctx, &cache, changed, Some(first));
    assert_eq!(execute_invocation(&ctx, command), Some(first));
}

#[test]
fn delayed_log_output_dedupes_before_queueing_and_keeps_admission() {
    let invocation_id = ModuleCommandInvocationId::new(NodeId(40), 1);
    let command = NodeId(101);
    let cache = OutputRuntimeCache {
        delay: 0.5,
        outputs: vec![output_target(command, true)],
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(&mut ctx, &cache, Vec::new(), Some(invocation_id));
    schedule.on_trigger_cached(&mut ctx, &cache, Vec::new(), Some(invocation_id));
    assert_eq!(schedule.pending.len(), 1);

    schedule.tick(&mut ctx, 0.5);
    let execute = execute_request(&ctx, command);
    assert_eq!(execute.invocation_id, Some(invocation_id));
    assert_eq!(
        execute.delivery_policy,
        ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted
    );
}

#[test]
fn nested_output_schedule_forwards_the_same_invocation() {
    let invocation_id = ModuleCommandInvocationId::new(NodeId(40), 9);
    let group = NodeId(101);
    let command = NodeId(102);
    let parent_cache = OutputRuntimeCache {
        outputs: vec![output_target(group, false)],
        ..OutputRuntimeCache::default()
    };
    let mut parent = OutputSchedule::default();
    let mut parent_ctx = process_ctx(1);

    parent.on_trigger_cached(
        &mut parent_ctx,
        &parent_cache,
        Vec::new(),
        Some(invocation_id),
    );
    let execute = execute_request(&parent_ctx, group);

    let child_cache = OutputRuntimeCache {
        outputs: vec![output_target(command, true)],
        ..OutputRuntimeCache::default()
    };
    let mut child = OutputSchedule::default();
    let mut child_ctx = process_ctx(2);
    child.on_trigger_cached(
        &mut child_ctx,
        &child_cache,
        execute.param_overrides,
        execute.invocation_id,
    );

    let child_execute = execute_request(&child_ctx, command);
    assert_eq!(child_execute.invocation_id, Some(invocation_id));
    assert_eq!(
        child_execute.delivery_policy,
        ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted
    );
}

fn output_target(node: NodeId, change_aware_log: bool) -> OutputRuntimeTarget {
    OutputRuntimeTarget {
        node,
        change_aware_log,
    }
}

fn process_ctx(tick: u64) -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick,
            micro: 0,
            seq: 0,
        },
    )
}

fn execute_request(
    ctx: &ProcessCtx,
    command: NodeId,
) -> module_command::ModuleCommandExecuteEvent {
    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("schedule should emit a command execute event")
        .edit
    else {
        panic!("schedule should emit a custom event");
    };
    module_command::command_execute_request(event, command)
        .expect("custom event should target the scheduled command")
}

fn execute_invocation(ctx: &ProcessCtx, command: NodeId) -> Option<ModuleCommandInvocationId> {
    execute_request(ctx, command).invocation_id
}

fn create_input_value_condition(engine: &mut crate::app::AppEngine, parent: NodeId) {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: crate::app::InputValueCondition::NODE_TYPE.to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        ack.success,
        "input value condition should attach to condition manager: {ack:?}"
    );
}

fn create_generic_log_output(engine: &mut crate::app::AppEngine, parent: NodeId) {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: crate::app::systems_alchemist_generic_commands::GenericLogCommand::NODE_TYPE
            .to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        ack.success,
        "generic log command should attach to outputs manager: {ack:?}"
    );
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "parameter change should apply: {ack:?}");
}

fn node_by_type(engine: &crate::app::AppEngine, node_type: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == node_type)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{node_type} should exist"))
}

fn output_control(engine: &crate::app::AppEngine, manager: NodeId, decl_id: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let advanced = snapshot
        .find_child_by_decl_id(manager, "advanced")
        .expect("advanced folder should exist");
    snapshot
        .find_child_by_decl_id(advanced, decl_id)
        .unwrap_or_else(|| panic!("{decl_id} output control should exist"))
}

fn assert_output_control_visibility(
    engine: &crate::app::AppEngine,
    manager: NodeId,
    decl_id: &str,
    expected: bool,
) {
    let snapshot = engine.process_tree_snapshot();
    let control = output_control(engine, manager, decl_id);
    assert_eq!(
        snapshot
            .node(control)
            .unwrap_or_else(|| panic!("{decl_id} output control should be in the snapshot"))
            .presentation
            .show_in_inspector_content,
        expected,
        "{decl_id} visibility should be {expected}"
    );
}

fn assert_operator_visibility(
    engine: &crate::app::AppEngine,
    manager: NodeId,
    expected: bool,
) {
    let snapshot = engine.process_tree_snapshot();
    let operator = snapshot
        .find_child_by_decl_id(manager, "operator")
        .expect("operator parameter should exist");
    assert_eq!(
        snapshot
            .node(operator)
            .expect("operator parameter should be in the snapshot")
            .presentation
            .show_in_inspector_content,
        expected
    );
}
