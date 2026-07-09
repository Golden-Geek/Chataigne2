use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::joycon::{
        joycon_led_state_enum_options, joycon_target_enum_options, JoyConSetLedRequest, JoyConVibrateRequest,
        JOYCON_LED_STATE_OFF, JOYCON_SET_LED_COMMAND_NODE_TYPE, JOYCON_TARGET_BOTH, JOYCON_VIBRATE_COMMAND_NODE_TYPE,
    },
    module_command,
};

macro_rules! joycon_command_node_impl {
    ($context:literal) => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            match event.kind {
                EventKind::ParamChanged { .. } => u32::MAX,
                _ => 0,
            }
        }

        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = snapshot_arc.as_ref();
            if !module_command::module_command_triggered(snapshot, self.id(), param) {
                return;
            }

            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                module_command::emit_module_command_request(ctx, snapshot, self.id(), self.get_type(), &payload)
            }) {
                golden_core::logerror!(format!("Failed to trigger {}: {error}", $context));
            }
        }

        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
            if !module_command::is_command_execute_request(&event, self.id()) {
                return;
            }
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = module_command::command_execute_snapshot(
                &event,
                snapshot_arc.as_ref(),
                self.id(),
            );
            let snapshot = snapshot.as_ref();
            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                module_command::emit_module_command_request(ctx, snapshot, self.id(), self.get_type(), &payload)
            }) {
                golden_core::logerror!(format!("Failed to execute {}: {error}", $context));
            }
        }
    };
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<JoyConVibrateRequest, String> {
        let target = command_enum_param(snapshot, self.id(), "target").unwrap_or_else(|| JOYCON_TARGET_BOTH.to_string());
        let frequency_hz = command_float_param(snapshot, self.id(), "frequency_hz").unwrap_or(300.0) as f32;
        let amplitude = command_float_param(snapshot, self.id(), "amplitude").unwrap_or(0.9) as f32;
        let duration_ms = command_int_param(snapshot, self.id(), "duration_ms")
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(60);

        Ok(JoyConVibrateRequest {
            target,
            frequency_hz,
            amplitude,
            duration_ms,
            description: "vibrate".to_string(),
        })
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<JoyConSetLedRequest, String> {
        Ok(JoyConSetLedRequest {
            target: command_enum_param(snapshot, self.id(), "target").unwrap_or_else(|| JOYCON_TARGET_BOTH.to_string()),
            led_1: command_enum_param(snapshot, self.id(), "led_1").unwrap_or_else(|| JOYCON_LED_STATE_OFF.to_string()),
            led_2: command_enum_param(snapshot, self.id(), "led_2").unwrap_or_else(|| JOYCON_LED_STATE_OFF.to_string()),
            led_3: command_enum_param(snapshot, self.id(), "led_3").unwrap_or_else(|| JOYCON_LED_STATE_OFF.to_string()),
            led_4: command_enum_param(snapshot, self.id(), "led_4").unwrap_or_else(|| JOYCON_LED_STATE_OFF.to_string()),
            description: "set leds".to_string(),
        })
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

fn command_enum_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

fn command_float_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<f64> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_float)
    })
}

fn command_int_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<i32> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_int)
    })
}

#[cfg(test)]
mod tests;
