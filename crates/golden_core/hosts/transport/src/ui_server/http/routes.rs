use super::Capability;

/// Routing and privilege selection share one closed set of operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HttpRoute {
    Discovery,
    Health,
    Bootstrap,
    Revoke,
    WebSocket,
    UserContexts,
    Snapshot,
    Replay,
    ReferenceTargets,
    ContextCandidates,
    ParamControlInfo,
    ScriptState,
    ScriptConfig,
    ScriptReload,
    ProjectNew,
    ProjectSave,
    ProjectLoad,
    ProjectUploadLoad,
    Intent,
    IntentBatch,
    Unknown,
}

impl HttpRoute {
    pub(super) fn resolve(method: &str, path: &str) -> Self {
        match (method, path) {
            ("GET", "/.well-known/chataigne") => Self::Discovery,
            ("GET", "/api/ui/health") => Self::Health,
            ("POST", "/api/ui/session") => Self::Bootstrap,
            ("POST", "/api/ui/session/revoke") => Self::Revoke,
            ("GET", "/api/ui/ws") => Self::WebSocket,
            ("GET", "/api/ui/user-contexts") => Self::UserContexts,
            ("POST", "/api/ui/snapshot") => Self::Snapshot,
            ("POST", "/api/ui/replay") => Self::Replay,
            ("POST", "/api/ui/reference-targets") => Self::ReferenceTargets,
            ("POST", "/api/ui/context-candidates") => Self::ContextCandidates,
            ("POST", "/api/ui/param-control-info") => Self::ParamControlInfo,
            ("POST", "/api/ui/script-state") => Self::ScriptState,
            ("POST", "/api/ui/script-config") => Self::ScriptConfig,
            ("POST", "/api/ui/script-reload") => Self::ScriptReload,
            ("POST", "/api/ui/project-new") => Self::ProjectNew,
            ("POST", "/api/ui/project-save") => Self::ProjectSave,
            ("POST", "/api/ui/project-load") => Self::ProjectLoad,
            ("POST", "/api/ui/project-upload-load") => Self::ProjectUploadLoad,
            ("POST", "/api/ui/intent") => Self::Intent,
            ("POST", "/api/ui/intent/batch") => Self::IntentBatch,
            _ => Self::Unknown,
        }
    }

    pub(super) const fn capability(self) -> Option<Capability> {
        match self {
            Self::Discovery | Self::Health | Self::Bootstrap | Self::Unknown => None,
            Self::Revoke => Some(Capability::Session),
            Self::WebSocket
            | Self::UserContexts
            | Self::Snapshot
            | Self::Replay
            | Self::ReferenceTargets
            | Self::ContextCandidates
            | Self::ParamControlInfo => Some(Capability::Observe),
            Self::Intent | Self::IntentBatch => Some(Capability::Author),
            Self::ScriptState | Self::ScriptConfig | Self::ScriptReload => Some(Capability::Scripts),
            Self::ProjectNew | Self::ProjectSave | Self::ProjectLoad | Self::ProjectUploadLoad => {
                Some(Capability::ProjectFiles)
            }
        }
    }
}
