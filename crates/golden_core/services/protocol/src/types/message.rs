use super::*;

/// Current UI protocol version.
pub const UI_PROTOCOL_VERSION: &str = "0.6.0";

/// Scope used by snapshot/event subscriptions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
pub enum UiSubscriptionScope {
    /// Full graph scope.
    #[default]
    WholeGraph,
    /// Subtree-limited scope rooted at `root` with `max_depth`.
    Subtree {
        /// Subtree root node id.
        root: NodeId,
        /// Maximum descendant depth (`0` means only root).
        max_depth: u32,
    },
}

/// Independently flow-controlled UI protocol plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UiDataPlane {
    /// Reliable document and presentation structure.
    Structure,
    /// Latest-wins runtime values.
    Value,
    /// Lossless triggers and application callbacks.
    Trigger,
    /// Latest-wins diagnostics and runtime observation.
    Observation,
    /// Static catalogs and schema descriptors.
    Catalog,
    /// Keyed authoring and runtime previews.
    Preview,
}

impl UiDataPlane {
    /// Every plane used by a complete workbench view.
    pub const ALL: [Self; 6] = [
        Self::Structure,
        Self::Value,
        Self::Trigger,
        Self::Observation,
        Self::Catalog,
        Self::Preview,
    ];

    /// Whether queued messages on this plane may be superseded by newer state.
    pub const fn is_latest_wins(self) -> bool {
        matches!(self, Self::Value | Self::Observation | Self::Preview)
    }
}

/// Per-view interest registered by one UI client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiInterest {
    /// Stable identifier for the consuming workbench view or panel.
    pub view_id: String,
    /// Graph scope observed by the view.
    pub scope: UiSubscriptionScope,
    /// Planes delivered to the view.
    pub planes: Vec<UiDataPlane>,
}

impl UiInterest {
    /// Creates an interest covering every protocol plane for `scope`.
    pub fn workbench(view_id: impl Into<String>, scope: UiSubscriptionScope) -> Self {
        Self {
            view_id: view_id.into(),
            scope,
            planes: UiDataPlane::ALL.to_vec(),
        }
    }

    /// Returns whether the view consumes `plane`.
    pub fn includes(&self, plane: UiDataPlane) -> bool {
        self.planes.contains(&plane)
    }
}

/// Lifecycle phase for one control request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UiControlPhase {
    /// The transport parsed the request.
    Received,
    /// The actor-owned control plane accepted the request.
    Accepted,
    /// The authoritative runtime applied the request.
    Applied,
    /// The authoritative runtime rejected the request.
    Rejected,
}

/// Lifecycle update for one control request or batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiControlUpdate {
    /// Client-generated request identifier.
    pub request_id: String,
    /// Current lifecycle phase.
    pub phase: UiControlPhase,
    /// Final single-intent acknowledgement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acknowledgement: Option<UiAck>,
    /// Final batch acknowledgements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledgements: Vec<UiAck>,
}

impl UiControlUpdate {
    /// Creates a non-final lifecycle update.
    pub fn pending(request_id: String, phase: UiControlPhase) -> Self {
        Self {
            request_id,
            phase,
            acknowledgement: None,
            acknowledgements: Vec::new(),
        }
    }
}

/// One independently flow-controlled delta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiPlaneDelta {
    /// Plane carrying the delta.
    pub plane: UiDataPlane,
    /// Existing revisioned event payload for that plane.
    pub batch: UiEventBatch,
}

/// Canonical client-to-runtime WebSocket message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiClientMessage {
    /// Starts or resumes a client session.
    Hello {
        /// Protocol version implemented by the client.
        protocol_version: String,
        /// Stable identifier for the browser tab.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_instance_id: Option<String>,
    },
    /// Registers or replaces one view interest.
    Subscribe {
        /// Client-local subscription identifier.
        subscription_id: String,
        /// View and plane interest.
        interest: UiInterest,
        /// Optional replay cursor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<EngineTime>,
    },
    /// Removes one view interest.
    Unsubscribe {
        /// Client-local subscription identifier.
        subscription_id: String,
    },
    /// Requests a scoped immutable snapshot over the live transport.
    Snapshot {
        /// Client-generated request identifier.
        request_id: String,
        /// Snapshot scope.
        scope: UiSubscriptionScope,
    },
    /// Requests a bounded scoped replay over the live transport.
    Replay {
        /// Client-generated request identifier.
        request_id: String,
        /// Replay scope.
        scope: UiSubscriptionScope,
        /// Optional replay cursor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<EngineTime>,
    },
    /// Submits one control intent.
    Intent {
        /// Client-generated request identifier.
        request_id: String,
        /// Typed edit intent.
        intent: Box<UiEditIntent>,
        /// Whether resulting events are echoed to the sender.
        #[serde(default)]
        include_self_events: bool,
    },
    /// Submits one ordered control batch.
    IntentBatch {
        /// Client-generated request identifier.
        request_id: String,
        /// Typed edit intents.
        intents: Vec<UiEditIntent>,
        /// Whether resulting events are echoed to the sender.
        #[serde(default)]
        include_self_events: bool,
    },
}

/// Canonical runtime-to-client WebSocket message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiServerMessage {
    /// Confirms the negotiated session.
    Hello {
        /// Protocol version implemented by the server.
        protocol_version: String,
        /// Server-assigned connection identifier.
        client_id: u64,
        /// Runtime session identifier used to detect restarts.
        session_id: String,
    },
    /// Returns a scoped immutable snapshot.
    Snapshot {
        /// Matching client request identifier.
        request_id: String,
        /// Requested snapshot.
        snapshot: Box<UiSnapshot>,
    },
    /// Returns a bounded scoped replay.
    Replay {
        /// Matching client request identifier.
        request_id: String,
        /// Requested replay batch.
        batch: UiEventBatch,
    },
    /// Delivers one atomic set of plane deltas from a server replay pass.
    Delta {
        /// Matching client subscription identifier.
        subscription_id: String,
        /// Plane-specific deltas staged together before the client may render.
        deltas: Vec<UiPlaneDelta>,
    },
    /// Advances a control request lifecycle.
    Control {
        /// Lifecycle update.
        update: UiControlUpdate,
    },
    /// Requires a scoped replay or snapshot refresh.
    ResyncRequired {
        /// Matching client subscription identifier.
        subscription_id: String,
        /// Affected plane, or every subscribed plane when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plane: Option<UiDataPlane>,
        /// Stable machine-readable reason.
        reason: String,
    },
    /// Reports a protocol or transport error.
    Error {
        /// Human-readable diagnostic.
        message: String,
        /// Matching request identifier when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
}

/// UI hello handshake.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiHello {
    /// Requested UI protocol version.
    pub protocol_version: String,
    /// Requested subscription scope.
    #[serde(default)]
    pub scope: UiSubscriptionScope,
    /// Optional replay cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<EngineTime>,
}

/// HTTP snapshot request payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiSnapshotRequest {
    /// Requested snapshot scope.
    #[serde(default)]
    pub scope: UiSubscriptionScope,
    /// Whether any active grouped edit should be cancelled before snapshotting.
    #[serde(default)]
    pub cancel_active_edit_session: bool,
}

/// HTTP replay request payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiReplayRequest {
    /// Requested replay scope.
    #[serde(default)]
    pub scope: UiSubscriptionScope,
    /// Optional replay cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<EngineTime>,
}

/// HTTP request payload for reference-target queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiReferenceTargetsRequest {
    /// Target reference parameter node id.
    pub param: NodeId,
}

/// HTTP request payload for user-context candidate queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiContextCandidatesRequest {
    /// Target parameter node id.
    pub param: NodeId,
}

/// HTTP request payload for parameter-control info queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiParamControlInfoRequest {
    /// Target parameter node id.
    pub param: NodeId,
}

/// HTTP request payload for script-state queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiScriptStateRequest {
    /// Target script node id.
    pub node: NodeId,
}

/// HTTP request payload for script-config updates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiScriptConfigRequest {
    /// Target script node id.
    pub node: NodeId,
    /// Replacement config payload.
    pub config: ScriptUiConfig,
    /// Whether the runtime should force an immediate reload.
    #[serde(default)]
    pub force_reload: bool,
}

/// HTTP request payload for script reloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiScriptReloadRequest {
    /// Target script node id.
    pub node: NodeId,
}

/// HTTP request payload carrying a project path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectPathRequest {
    /// Project path selected by the host.
    pub path: String,
    /// Optional project-owned UI state to write into the saved project document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_state: Option<serde_json::Value>,
    /// Whether to skip recoverable load-time rebuild problems after user confirmation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub recover: bool,
}

/// HTTP request payload for uploading a browser-selected project file before loading it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectUploadRequest {
    /// Original browser-side file name.
    pub file_name: String,
    /// Uploaded project document contents.
    pub contents: String,
    /// Whether to skip recoverable load-time rebuild problems after user confirmation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub recover: bool,
}

/// HTTP response payload describing a project load completed with recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectLoadRecoveryDto {
    /// Problems skipped while loading as much of the project as possible.
    pub problems: Vec<UiProjectLoadProblemDto>,
}

/// HTTP response payload for one skipped project-load problem.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectLoadProblemDto {
    /// Stable load stage identifier.
    pub stage: String,
    /// Human-readable problem description.
    pub message: String,
}

/// HTTP response payload carrying a resolved project path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectPathDto {
    /// Resolved project path on the host.
    pub path: String,
    /// Optional project-owned UI state loaded from the project document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_state: Option<serde_json::Value>,
    /// Recovery diagnostics when the user approved a partial project load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<UiProjectLoadRecoveryDto>,
}

/// UI-facing app project-file metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectFileSpec {
    /// Human-readable name for one project document, such as `Noisette`.
    pub display_name: String,
    /// Preferred filename extension without a leading dot.
    pub extension: String,
    /// Resolved host path for the currently open project document, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_path: Option<String>,
}

impl Default for UiProjectFileSpec {
    fn default() -> Self {
        Self {
            display_name: "Project".to_string(),
            extension: "json".to_string(),
            current_path: None,
        }
    }
}

impl UiProjectFileSpec {
    /// Builds UI project-file metadata from an app file spec and active host path.
    pub fn from_project_file_spec(spec: ProjectFileSpec, current_path: Option<String>) -> Self {
        Self {
            display_name: spec.normalized_display_name(),
            extension: spec.normalized_extension(),
            current_path,
        }
    }
}

impl From<ProjectFileSpec> for UiProjectFileSpec {
    fn from(spec: ProjectFileSpec) -> Self {
        Self::from_project_file_spec(spec, None)
    }
}
