use std::sync::Arc;

use golden_core::{
    edit::Edit,
    engine::EngineTime,
    events::{CustomEvent, Event, EventFrame},
    node::{Folder, Node, NodeId, NodeReference},
    parameter::{Parameter, ParameterChangeCheck, ParameterEventBehaviour, ParamValue},
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::{
    GENERIC_COMMAND_ITEM_KIND, GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE, GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE,
    GenericLogCommand, GenericLogRuntimeCache, LOG_INVOCATION_KEEPALIVE_TICKS, LOG_INVOCATION_STALE_TICKS,
    command_string_param_override, set_parameter_value, trigger_parameter,
};
use crate::app::module_command::{
    MODULE_COMMAND_EXECUTE_BATCH_TOPIC, MODULE_COMMAND_EXECUTE_TOPIC, ModuleCommandDeliveryPolicy,
    ModuleCommandExecuteBatchEvent, ModuleCommandExecuteEvent, ModuleCommandInvocationId, ModuleCommandParamOverride,
};

#[test]
fn parameter_commands_are_generic_and_do_not_require_a_node_module() {
    let generic_items = crate::app::declared_user_creatable_items(GENERIC_COMMAND_ITEM_KIND);
    for (node_type, label) in [
        (GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE, "Set Parameter"),
        (GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE, "Trigger Parameter"),
    ] {
        let item = generic_items
            .iter()
            .find(|item| item.node_type == node_type)
            .unwrap_or_else(|| panic!("{label} should be a generic command"));
        assert_eq!(item.label, label);
        assert!(
            crate::app::create_declared_user_item(node_type, GENERIC_COMMAND_ITEM_KIND).is_some(),
            "{label} should be creatable without a module"
        );
    }

    assert!(
        crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND)
            .iter()
            .all(|item| item.node_type != "node_module")
    );
}

#[test]
fn parameter_commands_queue_core_parameter_edits() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(
        Parameter::new(
            "Value Target",
            ParamValue::Float(0.0),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    engine.add_node(
        Parameter::new(
            "Trigger Target",
            ParamValue::Trigger(),
            ParameterChangeCheck::None,
        )
        .into(),
        None,
    );
    engine.apply_edits().expect("target parameters should attach");

    let value_target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Value Target")
        .map(|(id, node)| (id, node.node_data().meta.uuid))
        .expect("value target should exist");
    let trigger_target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Trigger Target")
        .map(|(id, node)| (id, node.node_data().meta.uuid))
        .expect("trigger target should exist");
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

    set_parameter_value(
        &mut ctx,
        snapshot.as_ref(),
        &ParamValue::Reference(NodeReference::new(value_target.1)),
        ParamValue::Float(0.75),
    )
    .expect("set command should resolve a stable parameter reference");
    trigger_parameter(
        &mut ctx,
        snapshot.as_ref(),
        &ParamValue::Reference(NodeReference::new(trigger_target.1)),
    )
    .expect("trigger command should resolve a stable trigger reference");

    assert!(matches!(
        &ctx.edits.pending[0].edit,
        Edit::SetParam {
            node,
            value: ParamValue::Float(0.75),
            behaviour: ParameterEventBehaviour::Coalesce,
        } if *node == value_target.0
    ));
    assert!(matches!(
        &ctx.edits.pending[1].edit,
        Edit::SetParam {
            node,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        } if *node == trigger_target.0
    ));
}

#[test]
fn cached_log_command_resolves_overrides_without_tree_snapshot() {
    let mut command = GenericLogCommand::create();
    command.cached_message = "original".to_owned();
    let message_param = NodeId(42);
    command.cached_message_param = Some(message_param);

    let event = CustomEvent::new(
        MODULE_COMMAND_EXECUTE_TOPIC,
        Some(command.id()),
        serde_json::to_value(ModuleCommandExecuteEvent {
            command_id: command.id(),
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: message_param,
                value: ParamValue::Str("lane message".to_owned()),
            }],
            invocation_id: None,
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .expect("execute event should serialize"),
    );
    let frame = EventFrame::from_shared(vec![Arc::new(Event::custom(
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
        event,
    ))]);

    assert!(!command.inbox_requires_tree_snapshot(&frame));
    let execute = crate::app::module_command::command_execute_request(
        match &frame[0].kind {
            golden_core::events::EventKind::Custom(event) => event,
            _ => panic!("expected custom execute event"),
        },
        command.id(),
    )
    .expect("execute event should decode");
    assert_eq!(
        command_string_param_override(&execute.param_overrides, message_param),
        Some("lane message".to_owned())
    );
}

#[test]
fn log_command_consumes_every_batched_execution_in_order() {
    let mut command = GenericLogCommand::create();
    let message_param = NodeId(42);
    command.cached_message_param = Some(message_param);
    let command_id = command.id();
    let before_id = golden_core::logger::records().last().map_or(0, |record| record.id);
    let messages = [
        "generic-batch-consumption-first",
        "generic-batch-consumption-second",
        "generic-batch-consumption-third",
    ];
    let executions = messages
        .iter()
        .map(|message| ModuleCommandExecuteEvent {
            command_id,
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: message_param,
                value: ParamValue::Str((*message).to_owned()),
            }],
            invocation_id: None,
            delivery_policy: ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted,
        })
        .collect();
    let event = CustomEvent::transient(
        MODULE_COMMAND_EXECUTE_BATCH_TOPIC,
        Some(command_id),
        serde_json::to_value(ModuleCommandExecuteBatchEvent { command_id, executions })
            .expect("execute batch should serialize"),
    );
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    command.on_custom_event(&mut ctx, event);

    let recorded = golden_core::logger::records()
        .into_iter()
        .filter(|record| record.id > before_id && record.origin == Some(command_id))
        .map(|record| record.message)
        .filter(|message| message.starts_with("generic-batch-consumption-"))
        .collect::<Vec<_>>();
    assert_eq!(recorded, messages);
}

#[test]
fn invocation_cache_is_change_aware_and_keeps_streams_distinct() {
    let emitter = NodeId(10);
    let first = ModuleCommandInvocationId::new(emitter, 1);
    let second = ModuleCommandInvocationId::new(emitter, 2);
    let mut cache = GenericLogRuntimeCache::default();

    assert!(cache.should_emit(first, "one", 1));
    assert!(!cache.should_emit(first, "one", 2));
    assert!(!cache.should_emit(first, "changed", 29));
    assert!(cache.should_emit(first, "changed", 31));
    assert!(cache.should_emit(second, "one", 32));
}

#[test]
fn invocation_budget_defers_without_recording_and_drains_next_tick() {
    let emitter = NodeId(10);
    let first = ModuleCommandInvocationId::new(emitter, 1);
    let deferred = ModuleCommandInvocationId::new(emitter, 2);
    let mut cache = GenericLogRuntimeCache::default();

    assert!(cache.should_emit(first, "first", 1));
    assert!(!cache.should_emit(deferred, "deferred", 1));
    assert!(!cache.records.contains_key(&deferred));
    assert!(cache.should_emit(deferred, "deferred", 2));
}

#[test]
fn invocation_keepalive_is_bounded_across_more_than_one_hundred_ticks() {
    let invocation = ModuleCommandInvocationId::new(NodeId(10), 1);
    let mut cache = GenericLogRuntimeCache::default();
    let emitted = (0..=(LOG_INVOCATION_KEEPALIVE_TICKS * 2))
        .filter(|tick| cache.should_emit(invocation, "steady", *tick))
        .collect::<Vec<_>>();

    assert_eq!(
        emitted,
        vec![0, LOG_INVOCATION_KEEPALIVE_TICKS, LOG_INVOCATION_KEEPALIVE_TICKS * 2]
    );
}

#[test]
fn invocation_cache_prunes_stale_streams_incrementally() {
    let emitter = NodeId(10);
    let stale = ModuleCommandInvocationId::new(emitter, 1);
    let current = ModuleCommandInvocationId::new(emitter, 2);
    let mut cache = GenericLogRuntimeCache::default();

    assert!(cache.should_emit(stale, "stale", 0));
    assert!(cache.should_emit(current, "current", LOG_INVOCATION_STALE_TICKS));

    assert!(!cache.records.contains_key(&stale));
    assert!(cache.records.contains_key(&current));
}
