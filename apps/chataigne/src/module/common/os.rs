use serde::{Deserialize, Serialize};

pub const OS_SHUTDOWN_COMMAND_NODE_TYPE: &str = "os_shutdown_command";
pub const OS_REBOOT_COMMAND_NODE_TYPE: &str = "os_reboot_command";
pub const OS_LOGOUT_COMMAND_NODE_TYPE: &str = "os_logout_command";
pub const OS_WAKE_ON_LAN_COMMAND_NODE_TYPE: &str = "os_wake_on_lan_command";

pub const OS_MODULE_COMMAND_TYPES: &[&str] = &[
    OS_SHUTDOWN_COMMAND_NODE_TYPE,
    OS_REBOOT_COMMAND_NODE_TYPE,
    OS_LOGOUT_COMMAND_NODE_TYPE,
    OS_WAKE_ON_LAN_COMMAND_NODE_TYPE,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OsControlRequest {
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WakeOnLanRequest {
    pub mac_address: String,
    pub broadcast_host: String,
    pub port: u16,
    pub description: String,
}