use golden_core::edit::Edit;
use golden_core::engine::EngineTime;
use golden_core::node::{Folder, Node, NodeId};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use golden_core::process_ctx::{ExecutionPhase, ProcessCtx};
use golden_core::ui_sync::UiEditIntent;

use super::{
    schedule::{MAX_RETAINED_OUTPUT_FANOUT_TARGETS, MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK},
    ConditionManager, ConsequencesManager, FilterChainManager, InputsManager, OutputGroup, OutputRuntimeCache,
    OutputRuntimeTarget, OutputSchedule, OutputsManager,
};
use crate::app::module_command::{
    self, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandInvocationId, ModuleCommandParamOverride,
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
    engine.apply_edits().expect("condition manager should attach");
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
    assert!(snapshot.find_child_by_decl_id(manager, "cancel_on_trigger").is_none());
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
        outputs: vec![output_target(first, false), output_target(second, false)].into(),
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(&mut ctx, invocation_id.emitter, &cache, Vec::new(), Some(invocation_id));
    assert_eq!(schedule.pending_len(), 2);
    assert!(ctx.edits.pending.is_empty());

    schedule.tick(&mut ctx, 0.5);
    assert_eq!(execute_invocation(&ctx, first), Some(invocation_id));
    ctx.edits.pending.clear();

    schedule.tick(&mut ctx, 0.5);
    assert_eq!(execute_invocation(&ctx, second), Some(invocation_id));
    assert!(schedule.is_empty());
}

#[test]
fn output_schedule_orders_overlapping_absolute_deadlines() {
    let first = NodeId(101);
    let second = NodeId(102);
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(
        &mut ctx,
        NodeId(40),
        &OutputRuntimeCache {
            delay: 1.0,
            outputs: vec![output_target(first, false)].into(),
            ..OutputRuntimeCache::default()
        },
        Vec::new(),
        None,
    );
    schedule.tick(&mut ctx, 0.25);
    schedule.on_trigger_cached(
        &mut ctx,
        NodeId(40),
        &OutputRuntimeCache {
            delay: 0.5,
            outputs: vec![output_target(second, false)].into(),
            ..OutputRuntimeCache::default()
        },
        Vec::new(),
        None,
    );

    schedule.tick(&mut ctx, 0.5);
    assert!(module_command::command_execute_request(execute_event(&ctx), second).is_some());
    ctx.edits.pending.clear();

    schedule.tick(&mut ctx, 0.25);
    assert!(module_command::command_execute_request(execute_event(&ctx), first).is_some());
    assert!(schedule.is_empty());
}

#[test]
fn output_schedule_carries_large_same_deadline_bursts_in_stable_bounded_batches() {
    const EXTRA: usize = 17;
    let container = NodeId(39);
    let emitter = NodeId(40);
    let command = NodeId(101);
    let scheduled = MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK * 2 + EXTRA;
    let cache = OutputRuntimeCache {
        delay: 1.0,
        outputs: vec![OutputRuntimeTarget {
            node: command,
            change_aware_log: false,
            batchable: true,
        }]
        .into(),
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    let executions = (0..scheduled)
        .map(|stream| ModuleCommandExecuteEvent {
            command_id: container,
            param_overrides: Vec::new(),
            invocation_id: Some(ModuleCommandInvocationId::new(emitter, stream as u64)),
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .collect::<Vec<_>>();
    for chunk in executions.chunks(module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS) {
        schedule.on_trigger_batch_cached(&mut ctx, container, &cache, chunk.to_vec());
    }
    assert_eq!(
        schedule.pending_len(),
        MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK,
        "the callback tick may only expand one bounded chunk"
    );

    let mut observed_streams = Vec::with_capacity(scheduled);
    for tick in 2..=5 {
        ctx.time.tick = tick;
        ctx.edits.pending.clear();
        schedule.tick(&mut ctx, if tick == 2 { 1.0 } else { 0.0 });
        assert!(
            ctx.edits.pending.len() <= 1,
            "one same-target run should serialize as at most one bounded batch"
        );
        if !ctx.edits.pending.is_empty() {
            let executions = execute_batch_requests(&ctx, command);
            assert!(
                executions.len() <= MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK,
                "one scheduler tick must never promote more than its explicit budget"
            );
            observed_streams.extend(
                executions
                    .into_iter()
                    .map(|execution| execution.invocation_id.expect("scheduled invocation").stream),
            );
        }
    }

    assert_eq!(observed_streams, (0..scheduled as u64).collect::<Vec<_>>());
    assert!(schedule.is_empty());
}

#[test]
fn output_batch_fanout_is_compact_serializable_bounded_and_complete() {
    const EXECUTION_COUNT: usize = 129;
    const TARGET_COUNT: usize = 17;

    let container = NodeId(40);
    let emitter = NodeId(41);
    let value_param = NodeId(42);
    let targets = (0..TARGET_COUNT)
        .map(|index| output_target(NodeId(100 + index as u64), false))
        .collect::<Vec<_>>();
    let cache = OutputRuntimeCache {
        outputs: targets.clone().into(),
        ..OutputRuntimeCache::default()
    };
    let executions = (0..EXECUTION_COUNT)
        .map(|stream| ModuleCommandExecuteEvent {
            command_id: container,
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: value_param,
                value: ParamValue::Str(format!("payload-{stream}")),
            }],
            invocation_id: Some(ModuleCommandInvocationId::new(emitter, stream as u64)),
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .collect::<Vec<_>>();
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);
    let mut observed = Vec::with_capacity(EXECUTION_COUNT * TARGET_COUNT);

    schedule.on_trigger_batch_cached(&mut ctx, container, &cache, executions);
    assert_output_fanout_tick_bound_and_collect(&ctx, value_param, &mut observed);
    assert_eq!(schedule.pending_fanout_job_count(), 1);
    assert_eq!(schedule.pending_fanout_stored_execution_count(), EXECUTION_COUNT);
    assert_eq!(schedule.pending_fanout_stored_target_count(), TARGET_COUNT);
    assert!(
        schedule.pending_fanout_stored_execution_count() + schedule.pending_fanout_stored_target_count()
            < EXECUTION_COUNT * TARGET_COUNT,
        "carry-over must store the two dimensions and cursors, not their Cartesian product"
    );

    let serialized = serde_json::to_vec(&schedule).expect("compact output continuation should serialize");
    let mut schedule: OutputSchedule =
        serde_json::from_slice(&serialized).expect("compact output continuation should deserialize");

    for tick in 2..=8 {
        if schedule.is_empty() {
            break;
        }
        ctx = process_ctx(tick);
        schedule.tick(&mut ctx, 0.0);
        assert_output_fanout_tick_bound_and_collect(&ctx, value_param, &mut observed);
    }

    let mut expected = Vec::with_capacity(EXECUTION_COUNT * TARGET_COUNT);
    for stream in 0..EXECUTION_COUNT {
        for target in &targets {
            expected.push((stream as u64, target.node, format!("payload-{stream}")));
        }
    }
    assert_eq!(observed, expected);
    assert!(schedule.is_empty(), "all compact carry-over work should complete");
    assert_eq!(schedule.rejected_fanout_execution_count(), 0);
}

#[test]
fn empty_cancel_trigger_invalidates_delayed_cells_still_in_compact_fanout() {
    let owner = NodeId(40);
    let targets = (0..=MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK)
        .map(|index| output_target(NodeId(100 + index as u64), false))
        .collect::<Vec<_>>();
    let delayed_cache = OutputRuntimeCache {
        delay: 1.0,
        outputs: targets.into(),
        ..OutputRuntimeCache::default()
    };
    let empty_cancel_cache = OutputRuntimeCache {
        cancel_on_trigger: true,
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(&mut ctx, owner, &delayed_cache, Vec::new(), None);
    assert_eq!(schedule.pending_len(), MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK);
    assert_eq!(schedule.pending_fanout_job_count(), 1);

    schedule.on_trigger_cached(&mut ctx, owner, &empty_cancel_cache, Vec::new(), None);
    assert_eq!(
        schedule.pending_len(),
        0,
        "the cancellation must clear materialized delays"
    );

    ctx = process_ctx(2);
    schedule.tick(&mut ctx, 1.0);
    assert!(
        ctx.edits.pending.is_empty(),
        "the cancellation barrier must also suppress delayed cells still represented by an older cursor"
    );
    assert!(schedule.is_empty());
}

#[test]
fn output_fanout_rejects_a_target_snapshot_above_its_memory_bound() {
    let owner = NodeId(40);
    let targets = (0..=MAX_RETAINED_OUTPUT_FANOUT_TARGETS)
        .map(|index| output_target(NodeId(100 + index as u64), false))
        .collect::<Vec<_>>();
    let cache = OutputRuntimeCache {
        outputs: targets.into(),
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(&mut ctx, owner, &cache, Vec::new(), None);

    assert!(ctx.edits.pending.is_empty());
    assert!(schedule.is_empty());
    assert_eq!(schedule.rejected_fanout_execution_count(), 1);
}

#[test]
fn immediate_log_output_rejects_unchanged_streams_before_emitting_events() {
    let emitter = NodeId(40);
    let first = ModuleCommandInvocationId::new(emitter, 1);
    let deferred = ModuleCommandInvocationId::new(emitter, 2);
    let command = NodeId(101);
    let cache = OutputRuntimeCache {
        outputs: vec![output_target(command, true)].into(),
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);
    let original = vec![ModuleCommandParamOverride {
        param_id: NodeId(201),
        value: ParamValue::Str("original".to_owned()),
    }];

    schedule.on_trigger_cached(&mut ctx, emitter, &cache, original.clone(), Some(first));
    let execute = execute_request(&ctx, command);
    assert_eq!(execute.invocation_id, Some(first));
    assert_eq!(
        execute.delivery_policy,
        ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted
    );
    ctx.edits.pending.clear();

    schedule.on_trigger_cached(&mut ctx, emitter, &cache, original.clone(), Some(first));
    assert!(ctx.edits.pending.is_empty());
    schedule.on_trigger_cached(&mut ctx, emitter, &cache, original.clone(), Some(deferred));
    assert!(ctx.edits.pending.is_empty());

    ctx.time.tick = 2;
    schedule.on_trigger_cached(&mut ctx, emitter, &cache, original.clone(), Some(deferred));
    assert_eq!(execute_invocation(&ctx, command), Some(deferred));
    ctx.edits.pending.clear();

    ctx.time.tick = 31;
    let changed = vec![ModuleCommandParamOverride {
        param_id: NodeId(201),
        value: ParamValue::Str("changed".to_owned()),
    }];
    schedule.on_trigger_cached(&mut ctx, emitter, &cache, changed, Some(first));
    assert_eq!(execute_invocation(&ctx, command), Some(first));
}

#[test]
fn immediate_log_output_forwards_an_ordered_batch_in_one_event() {
    let container = NodeId(40);
    let command = NodeId(101);
    let message_param = NodeId(201);
    let cache = OutputRuntimeCache {
        outputs: vec![output_target(command, true)].into(),
        ..OutputRuntimeCache::default()
    };
    let executions = ["first", "second", "third"]
        .into_iter()
        .map(|message| ModuleCommandExecuteEvent {
            command_id: container,
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: message_param,
                value: ParamValue::Str(message.to_owned()),
            }],
            invocation_id: None,
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .collect();
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_batch_cached(&mut ctx, container, &cache, executions);

    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "all immediate executions for one log child should share one event"
    );
    let forwarded = execute_batch_requests(&ctx, command);
    assert_eq!(
        forwarded
            .iter()
            .map(|execute| {
                execute.param_overrides[0]
                    .value
                    .as_str()
                    .expect("message override should remain a string")
            })
            .collect::<Vec<_>>(),
        vec!["first", "second", "third"]
    );
    assert!(
        forwarded.iter().all(|execute| execute.command_id == command),
        "forwarded entries should target the immediate log child"
    );
}

#[test]
fn immediate_log_batch_retains_change_aware_admission_and_identity() {
    let container = NodeId(40);
    let command = NodeId(101);
    let invocation_id = ModuleCommandInvocationId::new(container, 7);
    let cache = OutputRuntimeCache {
        outputs: vec![output_target(command, true)].into(),
        ..OutputRuntimeCache::default()
    };
    let execution = ModuleCommandExecuteEvent {
        command_id: container,
        param_overrides: Vec::new(),
        invocation_id: Some(invocation_id),
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_batch_cached(&mut ctx, container, &cache, vec![execution.clone(), execution]);

    let forwarded = execute_batch_requests(&ctx, command);
    assert_eq!(
        forwarded.len(),
        1,
        "unchanged streams should be admitted before batch serialization"
    );
    assert_eq!(forwarded[0].invocation_id, Some(invocation_id));
    assert_eq!(
        forwarded[0].delivery_policy,
        ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted
    );
}

#[test]
fn immediate_log_batching_preserves_mixed_output_event_order() {
    let container = NodeId(40);
    let first_log = NodeId(101);
    let non_log = NodeId(102);
    let second_log = NodeId(103);
    let message_param = NodeId(201);
    let cache = OutputRuntimeCache {
        outputs: vec![
            output_target(first_log, true),
            output_target(non_log, false),
            output_target(second_log, true),
        ]
        .into(),
        ..OutputRuntimeCache::default()
    };
    let executions = ["first", "second"]
        .into_iter()
        .map(|message| ModuleCommandExecuteEvent {
            command_id: container,
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: message_param,
                value: ParamValue::Str(message.to_owned()),
            }],
            invocation_id: None,
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .collect();
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_batch_cached(&mut ctx, container, &cache, executions);

    let emitted = ctx
        .edits
        .pending
        .iter()
        .map(|pending| {
            let Edit::EmitCustomEvent { event } = &pending.edit else {
                panic!("output schedule should emit only custom events");
            };
            let command = event
                .origin
                .expect("command execute events should identify their target");
            let (batched, executions) =
                if let Some(executions) = module_command::command_execute_batch_requests(event, command) {
                    (true, executions)
                } else {
                    (
                        false,
                        vec![module_command::command_execute_request(event, command)
                            .expect("event should contain a command execution")],
                    )
                };
            assert_eq!(
                executions.len(),
                1,
                "intervening targets must terminate the current log run"
            );
            let message = executions[0].param_overrides[0]
                .value
                .as_str()
                .expect("message override should remain a string");
            (command, message, batched)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        emitted,
        vec![
            (first_log, "first".to_owned(), true),
            (non_log, "first".to_owned(), false),
            (second_log, "first".to_owned(), true),
            (first_log, "second".to_owned(), true),
            (non_log, "second".to_owned(), false),
            (second_log, "second".to_owned(), true),
        ],
        "batching must preserve the original execution-major output order"
    );
}

#[test]
fn delayed_log_output_dedupes_before_queueing_and_keeps_admission() {
    let invocation_id = ModuleCommandInvocationId::new(NodeId(40), 1);
    let command = NodeId(101);
    let cache = OutputRuntimeCache {
        delay: 0.5,
        outputs: vec![output_target(command, true)].into(),
        ..OutputRuntimeCache::default()
    };
    let mut schedule = OutputSchedule::default();
    let mut ctx = process_ctx(1);

    schedule.on_trigger_cached(&mut ctx, invocation_id.emitter, &cache, Vec::new(), Some(invocation_id));
    schedule.on_trigger_cached(&mut ctx, invocation_id.emitter, &cache, Vec::new(), Some(invocation_id));
    assert_eq!(schedule.pending_len(), 1);

    schedule.tick(&mut ctx, 0.5);
    let execute = execute_batch_requests(&ctx, command)
        .into_iter()
        .next()
        .expect("one admitted delayed log execution should fire");
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
        outputs: vec![output_target(group, false)].into(),
        ..OutputRuntimeCache::default()
    };
    let mut parent = OutputSchedule::default();
    let mut parent_ctx = process_ctx(1);

    parent.on_trigger_cached(
        &mut parent_ctx,
        invocation_id.emitter,
        &parent_cache,
        Vec::new(),
        Some(invocation_id),
    );
    let execute = execute_request(&parent_ctx, group);

    let child_cache = OutputRuntimeCache {
        outputs: vec![output_target(command, true)].into(),
        ..OutputRuntimeCache::default()
    };
    let mut child = OutputSchedule::default();
    let mut child_ctx = process_ctx(2);
    child.on_trigger_cached(
        &mut child_ctx,
        group,
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
        batchable: change_aware_log,
    }
}

fn process_ctx(tick: u64) -> ProcessCtx {
    ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick, micro: 0, seq: 0 })
}

fn execute_request(ctx: &ProcessCtx, command: NodeId) -> module_command::ModuleCommandExecuteEvent {
    module_command::command_execute_request(execute_event(ctx), command)
        .expect("custom event should target the scheduled command")
}

fn execute_event(ctx: &ProcessCtx) -> &golden_core::events::CustomEvent {
    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("schedule should emit a command execute event")
        .edit
    else {
        panic!("schedule should emit a custom event");
    };
    event
}

fn execute_invocation(ctx: &ProcessCtx, command: NodeId) -> Option<ModuleCommandInvocationId> {
    execute_request(ctx, command).invocation_id
}

fn execute_batch_requests(ctx: &ProcessCtx, command: NodeId) -> Vec<ModuleCommandExecuteEvent> {
    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("schedule should emit a command execute batch")
        .edit
    else {
        panic!("schedule should emit a custom event");
    };
    module_command::command_execute_batch_requests(event, command)
        .expect("custom event should contain the scheduled command batch")
}

fn assert_output_fanout_tick_bound_and_collect(
    ctx: &ProcessCtx,
    value_param: NodeId,
    observed: &mut Vec<(u64, NodeId, String)>,
) {
    assert!(
        ctx.edits.pending.len() <= MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK,
        "one callback or scheduler tick must emit at most its strict work budget"
    );
    for pending in &ctx.edits.pending {
        let Edit::EmitCustomEvent { event } = &pending.edit else {
            panic!("output fan-out should emit only custom events");
        };
        let target = event.origin.expect("command execution should identify its target");
        let execution = module_command::command_execute_request(event, target)
            .expect("non-batchable output should retain one execution payload");
        assert_eq!(execution.command_id, target);
        let value = execution
            .param_overrides
            .iter()
            .find(|param| param.param_id == value_param)
            .and_then(|param| param.value.as_str())
            .expect("fan-out should preserve the string override")
            .to_owned();
        let stream = execution
            .invocation_id
            .expect("fan-out should preserve invocation identity")
            .stream;
        observed.push((stream, target, value));
    }
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
        node_type: crate::app::systems_alchemist_generic_commands::GenericLogCommand::NODE_TYPE.to_owned(),
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

fn assert_output_control_visibility(engine: &crate::app::AppEngine, manager: NodeId, decl_id: &str, expected: bool) {
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

fn assert_operator_visibility(engine: &crate::app::AppEngine, manager: NodeId, expected: bool) {
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
