use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};

use crate::app::{
    module::common::buttplug::{
        buttplug_output_enum_options, BUTTPLUG_OUTPUT_VIBRATE, BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE,
        BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE, BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE,
        BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE, BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE, BUTTPLUG_TARGET_SELECTED,
    },
    module_command,
};

macro_rules! buttplug_command_node_impl {
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
    if let Err(error) = module_command::CapturedInvocation::capture(&snapshot, command, event, &["buttplug_module"])
        .and_then(|invocation| crate::app::queue_buttplug_command(ctx, invocation))
    {
        golden_core::logerror!(format!("Buttplug command was not admitted: {error}"));
    }
}

#[node("buttplug_set_output_command", label = "Set Output")]
#[children(
    target: String = BUTTPLUG_TARGET_SELECTED.to_string() (
        label = "Device",
        description = "Target device. Use selected, all, a Buttplug device index, or a device name."
    );
    output: Enum = BUTTPLUG_OUTPUT_VIBRATE (
        label = "Output",
        description = "Buttplug output capability to drive.",
        enum_options = buttplug_output_enum_options()
    );
    value: f64 = 0.0 [0.0..1.0] (
        label = "Value",
        description = "Output value as a normalized 0.0-1.0 percentage."
    );
    duration_ms: i32 = 1000 [1..600000] (
        label = "Duration",
        description = "Duration in milliseconds for Position With Duration outputs.",
        widget = "text"
    );
)]
pub struct ButtplugSetOutputCommand {
    base: crate::app::ModuleCommandBase,
}

impl ButtplugSetOutputCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "buttplug_set_output_command",
    via = base,
    from_struct
)]
impl Node for ButtplugSetOutputCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE).then(Self::create)
    }

    buttplug_command_node_impl!("Buttplug set output command");
}

#[node("buttplug_stop_device_command", label = "Stop Device")]
#[children(
    target: String = BUTTPLUG_TARGET_SELECTED.to_string() (
        label = "Device",
        description = "Target device. Use selected, all, a Buttplug device index, or a device name."
    );
)]
pub struct ButtplugStopDeviceCommand {
    base: crate::app::ModuleCommandBase,
}

impl ButtplugStopDeviceCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "buttplug_stop_device_command",
    via = base,
    from_struct
)]
impl Node for ButtplugStopDeviceCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE).then(Self::create)
    }

    buttplug_command_node_impl!("Buttplug stop device command");
}

#[node("buttplug_stop_all_devices_command", label = "Stop All Devices")]
pub struct ButtplugStopAllDevicesCommand {
    base: crate::app::ModuleCommandBase,
}

impl ButtplugStopAllDevicesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "buttplug_stop_all_devices_command",
    via = base,
    from_struct
)]
impl Node for ButtplugStopAllDevicesCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE).then(Self::create)
    }

    buttplug_command_node_impl!("Buttplug stop all devices command");
}

#[node("buttplug_start_scanning_command", label = "Start Scanning")]
pub struct ButtplugStartScanningCommand {
    base: crate::app::ModuleCommandBase,
}

impl ButtplugStartScanningCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "buttplug_start_scanning_command",
    via = base,
    from_struct
)]
impl Node for ButtplugStartScanningCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE).then(Self::create)
    }

    buttplug_command_node_impl!("Buttplug start scanning command");
}

#[node("buttplug_stop_scanning_command", label = "Stop Scanning")]
pub struct ButtplugStopScanningCommand {
    base: crate::app::ModuleCommandBase,
}

impl ButtplugStopScanningCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "buttplug_stop_scanning_command",
    via = base,
    from_struct
)]
impl Node for ButtplugStopScanningCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE).then(Self::create)
    }

    buttplug_command_node_impl!("Buttplug stop scanning command");
}

#[cfg(test)]
mod tests;
