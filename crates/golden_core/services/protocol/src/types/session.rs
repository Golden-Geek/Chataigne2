use super::*;

/// Independent transport privileges. Authoring can invoke application behavior through graph edits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
pub struct UiSessionCapabilities {
    /// Read graph snapshots, subscriptions and catalogs.
    pub observe: bool,
    /// Submit graph edits, commands and history operations, including their application side effects.
    pub author: bool,
    /// Invoke direct project creation, upload, load and filesystem save operations.
    pub project_files: bool,
    /// Read or change script configuration and invoke script reload operations.
    pub scripts: bool,
}

impl UiSessionCapabilities {
    /// The complete capability set; the host decides whether to grant it.
    pub const ALL: Self = Self {
        observe: true,
        author: true,
        project_files: true,
        scripts: true,
    };

    /// Whether this set requests only privileges contained in `allowed`.
    pub const fn is_subset_of(self, allowed: Self) -> bool {
        (!self.observe || allowed.observe)
            && (!self.author || allowed.author)
            && (!self.project_files || allowed.project_files)
            && (!self.scripts || allowed.scripts)
    }
}

/// Password-free session bootstrap from an admitted browser origin or trusted native peer.
#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UiSessionRequest {
    /// Explicit subset to request, or null for the host's configured grant.
    #[serde(default)]
    pub capabilities: Option<UiSessionCapabilities>,
}

/// Ephemeral host-owned credential; keep it out of URLs and logs.
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct UiSessionResponse {
    /// Opaque bearer token, also carried in the WebSocket credential subprotocol.
    pub token: String,
    /// Privileges actually granted by this host.
    pub capabilities: UiSessionCapabilities,
    /// Remaining credential lifetime in milliseconds.
    pub expires_in_ms: u32,
    /// Canonical protocol version for native consumers.
    pub protocol_version: String,
    /// WebSocket protocol offers prepared by the host, including its opaque credential offer.
    pub websocket_protocols: Vec<String>,
}

impl std::fmt::Debug for UiSessionResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UiSessionResponse")
            .field("token", &"[redacted]")
            .field("capabilities", &self.capabilities)
            .field("expires_in_ms", &self.expires_in_ms)
            .field("protocol_version", &self.protocol_version)
            .finish()
    }
}
