use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::os::{
        OsControlRequest, WakeOnLanRequest, OS_LOGOUT_COMMAND_NODE_TYPE,
        OS_REBOOT_COMMAND_NODE_TYPE, OS_SHUTDOWN_COMMAND_NODE_TYPE,
        OS_WAKE_ON_LAN_COMMAND_NODE_TYPE,
    },
    module_command,
};

macro_rules! os_command_node_impl {
    ($payload_method:ident, $context:literal) => {
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

            let payload = self.$payload_method();

            if let Err(error) = module_command::emit_module_command_request(
                ctx,
                snapshot,
                self.id(),
                self.get_type(),
                &payload,
            ) {
                golden_core::logerror!(format!("Failed to trigger {}: {error}", $context));
            }
        }
    };
}

#[node("os_shutdown_command", label = "Shutdown")]
pub struct OsShutdownCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsShutdownCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self) -> OsControlRequest {
        OsControlRequest {
            description: "shutdown host".to_string(),
        }
    }
}

#[golden_core::item(
    "module_command",
    node = "os_shutdown_command",
    via = base,
    from_struct
)]
impl Node for OsShutdownCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == OS_SHUTDOWN_COMMAND_NODE_TYPE).then(Self::create)
    }

    os_command_node_impl!(request_payload, "OS shutdown command");
}

#[node("os_reboot_command", label = "Reboot")]
pub struct OsRebootCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsRebootCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self) -> OsControlRequest {
        OsControlRequest {
            description: "reboot host".to_string(),
        }
    }
}

#[golden_core::item(
    "module_command",
    node = "os_reboot_command",
    via = base,
    from_struct
)]
impl Node for OsRebootCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == OS_REBOOT_COMMAND_NODE_TYPE).then(Self::create)
    }

    os_command_node_impl!(request_payload, "OS reboot command");
}

#[node("os_logout_command", label = "Logout")]
pub struct OsLogoutCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsLogoutCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self) -> OsControlRequest {
        OsControlRequest {
            description: "logout host user".to_string(),
        }
    }
}

#[golden_core::item(
    "module_command",
    node = "os_logout_command",
    via = base,
    from_struct
)]
impl Node for OsLogoutCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == OS_LOGOUT_COMMAND_NODE_TYPE).then(Self::create)
    }

    os_command_node_impl!(request_payload, "OS logout command");
}

#[node("os_wake_on_lan_command", label = "Wake On Lan")]
#[children(
    mac_address: String = String::new() (
        label = "MAC Address",
        description = "Target hardware MAC address. Common separators such as :, -, or spaces are accepted."
    );
    broadcast_host: String = "255.255.255.255".to_string() (
        label = "Broadcast Host",
        description = "Broadcast address used to send the Wake-on-LAN packet."
    );
    port: i32 = 9 [0..65535] (
        label = "Port",
        description = "UDP port used for the Wake-on-LAN magic packet.",
        widget = "text"
    );
)]
pub struct OsWakeOnLanCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsWakeOnLanCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> WakeOnLanRequest {
        WakeOnLanRequest {
            mac_address: command_string_param(snapshot, self.id(), "mac_address").unwrap_or_default(),
            broadcast_host: command_string_param(snapshot, self.id(), "broadcast_host")
                .unwrap_or_else(|| "255.255.255.255".to_string()),
            port: command_int_param(snapshot, self.id(), "port").unwrap_or(9).clamp(0, 65535) as u16,
            description: "send Wake-on-LAN packet".to_string(),
        }
    }
}

#[golden_core::item(
    "module_command",
    node = "os_wake_on_lan_command",
    via = base,
    from_struct
)]
impl Node for OsWakeOnLanCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == OS_WAKE_ON_LAN_COMMAND_NODE_TYPE).then(Self::create)
    }

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

        let payload = self.request_payload(snapshot);
        if let Err(error) = module_command::emit_module_command_request(
            ctx,
            snapshot,
            self.id(),
            self.get_type(),
            &payload,
        ) {
            golden_core::logerror!(format!("Failed to trigger Wake-on-LAN command: {error}"));
        }
    }
}

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
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