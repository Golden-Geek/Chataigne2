use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::buttplug::{
        buttplug_output_enum_options, ButtplugControlRequest, ButtplugSetOutputRequest,
        ButtplugTargetRequest, BUTTPLUG_OUTPUT_VIBRATE, BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE,
        BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE, BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE,
        BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE, BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE,
        BUTTPLUG_TARGET_SELECTED,
    },
    module_command,
};

macro_rules! buttplug_command_node_impl {
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
                module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
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
                module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
            }) {
                golden_core::logerror!(format!("Failed to execute {}: {error}", $context));
            }
        }
    };
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<ButtplugSetOutputRequest, String> {
        let target = command_string_param(snapshot, self.id(), "target")
            .unwrap_or_else(|| BUTTPLUG_TARGET_SELECTED.to_string());
        let output =
            command_enum_param(snapshot, self.id(), "output").unwrap_or_else(|| BUTTPLUG_OUTPUT_VIBRATE.to_string());
        let value = command_float_param(snapshot, self.id(), "value").unwrap_or(0.0);
        let duration_ms = command_int_param(snapshot, self.id(), "duration_ms")
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(1000);

        Ok(ButtplugSetOutputRequest {
            target,
            output,
            value,
            duration_ms,
            description: "set output".to_string(),
        })
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<ButtplugTargetRequest, String> {
        Ok(ButtplugTargetRequest {
            target: command_string_param(snapshot, self.id(), "target")
                .unwrap_or_else(|| BUTTPLUG_TARGET_SELECTED.to_string()),
            description: "stop device".to_string(),
        })
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

    fn request_payload(&self, _snapshot: &ProcessTreeSnapshot) -> Result<ButtplugControlRequest, String> {
        Ok(ButtplugControlRequest {
            description: "stop all devices".to_string(),
        })
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

    fn request_payload(&self, _snapshot: &ProcessTreeSnapshot) -> Result<ButtplugControlRequest, String> {
        Ok(ButtplugControlRequest {
            description: "start scanning".to_string(),
        })
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

    fn request_payload(&self, _snapshot: &ProcessTreeSnapshot) -> Result<ButtplugControlRequest, String> {
        Ok(ButtplugControlRequest {
            description: "stop scanning".to_string(),
        })
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

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
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
