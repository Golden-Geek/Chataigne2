use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

use crate::app::{
    module::common::os::{
        OS_LOGOUT_COMMAND_NODE_TYPE, OS_REBOOT_COMMAND_NODE_TYPE, OS_SHUTDOWN_COMMAND_NODE_TYPE,
        OS_WAKE_ON_LAN_COMMAND_NODE_TYPE,
    },
    module_command,
};

macro_rules! os_command_node_impl {
    () => {
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
    if let Err(error) = module_command::CapturedInvocation::capture(&snapshot, command, event, &["os_module"])
        .and_then(|invocation| crate::app::queue_os_command(ctx, invocation))
    {
        golden_core::logerror!(format!("OS command was not admitted: {error}"));
    }
}

#[node("os_shutdown_command", label = "Shutdown")]
pub struct OsShutdownCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsShutdownCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
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

    os_command_node_impl!();
}

#[node("os_reboot_command", label = "Reboot")]
pub struct OsRebootCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsRebootCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
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

    os_command_node_impl!();
}

#[node("os_logout_command", label = "Logout")]
pub struct OsLogoutCommand {
    base: crate::app::ModuleCommandBase,
}

impl OsLogoutCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
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

    os_command_node_impl!();
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

    os_command_node_impl!();
}
