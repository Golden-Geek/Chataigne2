use std::sync::Arc;

use golden_core::{
    engine::EngineTime,
    events::{CustomEvent, Event, EventFrame},
    node::Node,
    parameter::ParamValue,
};

use super::GenericLogCommand;
use crate::app::module_command::{
    ModuleCommandExecuteEvent, ModuleCommandParamOverride, MODULE_COMMAND_EXECUTE_TOPIC,
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
