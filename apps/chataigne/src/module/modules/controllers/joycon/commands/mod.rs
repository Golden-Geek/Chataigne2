use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};

use crate::app::{
    module::common::joycon::{
        joycon_led_state_enum_options, joycon_target_enum_options, JOYCON_LED_STATE_OFF,
        JOYCON_SET_LED_COMMAND_NODE_TYPE, JOYCON_TARGET_BOTH, JOYCON_VIBRATE_COMMAND_NODE_TYPE,
    },
    module_command,
};

macro_rules! joycon_command_node_impl {
    ($context:literal) => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            if matches!(event.kind, EventKind::ParamChanged { .. }) {
                u32::MAX
            } else {
                0
            }
        }
        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
            if ctx
                .tree_snapshot()
                .is_some_and(|snapshot| module_command::module_command_triggered(snapshot, self.id(), param))
            {
                run(ctx, self.id(), None);
            }
        }
        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
            if event.topic == module_command::MODULE_COMMAND_EXECUTE_TOPIC
                && event
                    .payload
                    .get("command_id")
                    .and_then(serde_json::Value::as_u64)
                    == Some(self.id().0)
            {
                run(ctx, self.id(), Some(event));
            }
        }
    };
}
fn run(ctx: &mut ProcessCtx, command: NodeId, event: Option<golden_core::events::CustomEvent>) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    if let Err(error) = module_command::CapturedInvocation::capture(&snapshot, command, event, &["joycon_module"])
        .and_then(|invocation| crate::app::queue_joycon_command(ctx, invocation))
    {
        golden_core::logerror!(format!("Joy-Con command was not admitted: {error}"));
    }
}

#[node("joycon_vibrate_command", label = "Vibrate")]
#[children(
    target: Enum = JOYCON_TARGET_BOTH (
        label = "Controller",
        description = "Controller slot to drive. Both sends the command to every connected Joy-Con slot.",
        enum_options = joycon_target_enum_options()
    );
    frequency_hz: f64 = 300.0 [0.0..1252.0] (
        label = "Frequency",
        description = "Rumble frequency in hertz. Joy-Con saturates values outside its supported range.",
        widget = "text"
    );
    amplitude: f64 = 0.9 [0.0..1.799] (
        label = "Amplitude",
        description = "Rumble amplitude. Values above 1.003 are not considered actuator-safe.",
        widget = "text"
    );
    duration_ms: i32 = 60 [1..600000] (
        label = "Duration",
        description = "How long the rumble should be held before sending a stop command.",
        widget = "text"
    );
)]
pub struct JoyConVibrateCommand {
    base: crate::app::ModuleCommandBase,
}

impl JoyConVibrateCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "joycon_vibrate_command",
    via = base,
    from_struct
)]
impl Node for JoyConVibrateCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == JOYCON_VIBRATE_COMMAND_NODE_TYPE).then(Self::create)
    }

    joycon_command_node_impl!("Joy-Con vibrate command");
}

#[node("joycon_set_led_command", label = "Set LED")]
#[children(
    target: Enum = JOYCON_TARGET_BOTH (
        label = "Controller",
        description = "Controller slot to drive. Both sends the command to every connected Joy-Con slot.",
        enum_options = joycon_target_enum_options()
    );
    led_1: Enum = JOYCON_LED_STATE_OFF (
        label = "LED 1",
        description = "Player light state for the left-most Joy-Con LED.",
        enum_options = joycon_led_state_enum_options()
    );
    led_2: Enum = JOYCON_LED_STATE_OFF (
        label = "LED 2",
        description = "Player light state for the second Joy-Con LED.",
        enum_options = joycon_led_state_enum_options()
    );
    led_3: Enum = JOYCON_LED_STATE_OFF (
        label = "LED 3",
        description = "Player light state for the third Joy-Con LED.",
        enum_options = joycon_led_state_enum_options()
    );
    led_4: Enum = JOYCON_LED_STATE_OFF (
        label = "LED 4",
        description = "Player light state for the right-most Joy-Con LED.",
        enum_options = joycon_led_state_enum_options()
    );
)]
pub struct JoyConSetLedCommand {
    base: crate::app::ModuleCommandBase,
}

impl JoyConSetLedCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "joycon_set_led_command",
    via = base,
    from_struct
)]
impl Node for JoyConSetLedCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == JOYCON_SET_LED_COMMAND_NODE_TYPE).then(Self::create)
    }

    joycon_command_node_impl!("Joy-Con set-led command");
}

#[cfg(test)]
mod tests;
