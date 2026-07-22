use std::sync::Arc;

use golden_core::{
    engine::EngineTime,
    events::{CustomEvent, Event, EventFrame},
    node::{Node, NodeId},
    parameter::ParamValue,
};

use super::{
    GenericLogCommand, GenericLogRuntimeCache, LOG_INVOCATION_KEEPALIVE_TICKS,
    LOG_INVOCATION_STALE_TICKS,
};
use crate::app::module_command::{
    ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandInvocationId,
    ModuleCommandParamOverride, MODULE_COMMAND_EXECUTE_TOPIC,
};

#[test]
fn log_command_execute_with_overrides_requires_tree_snapshot_even_when_cached() {
    let mut command = GenericLogCommand::create();
    command.cached_message = "original".to_owned();

    let event = CustomEvent::new(
        MODULE_COMMAND_EXECUTE_TOPIC,
        Some(command.id()),
        serde_json::to_value(ModuleCommandExecuteEvent {
            command_id: command.id(),
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: command.id(),
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

    assert!(command.inbox_requires_tree_snapshot(&frame));
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

    assert_eq!(emitted, vec![0, LOG_INVOCATION_KEEPALIVE_TICKS, LOG_INVOCATION_KEEPALIVE_TICKS * 2]);
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
