use super::*;

/// Product identity advertised by a Golden application host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiApplicationInfoDto {
    /// Stable application identifier used to isolate client state.
    pub id: String,
    /// Human-readable product name.
    pub display_name: String,
    /// Product release version, separate from the wire protocol version.
    pub version: String,
    /// Directory name under the OS application-data root.
    pub data_directory_name: String,
}

/// Well-known document for discovering a Golden application's UI transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiDiscoveryDto {
    /// Discovery document schema version.
    pub version: u16,
    /// Stable transport service identifier.
    pub service: String,
    /// Relative readiness endpoint.
    pub health_path: String,
    /// Relative WebSocket endpoint.
    pub websocket_path: String,
    /// Whether endpoints resolve against the discovery document's origin.
    pub relative_endpoints: bool,
    /// Application identity supplied by the product lifecycle.
    pub application: UiApplicationInfoDto,
    /// Exact UI wire protocol version required by this host.
    pub protocol_version: String,
}
