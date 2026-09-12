use super::*;

/// Persisted subset of runtime node metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectNodeMeta {
    /// Declared id key under the parent scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decl_id: Option<DeclId>,
    /// Generated short name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Runtime enablement state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Whether this node can be disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_be_disabled: Option<bool>,
    /// User-visible label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// Canonical declaration-description key shared by repeated declared nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description_key: Option<Option<String>>,
    /// Canonical declaration description before any instance-level override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description: Option<Option<String>>,
    /// User tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// User-edit permissions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_permissions: Option<NodeUserPermissions>,
    /// Semantic hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<SemanticsHint>,
    /// Presentation hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationHint>,
}

impl ProjectNodeMeta {
    pub(crate) fn is_empty(&self) -> bool {
        self.decl_id.is_none()
            && self.short_name.is_none()
            && self.enabled.is_none()
            && self.can_be_disabled.is_none()
            && self.label.is_none()
            && self.description.is_none()
            && self.declared_description_key.is_none()
            && self.declared_description.is_none()
            && self.tags.is_none()
            && self.user_permissions.is_none()
            && self.semantics.is_none()
            && self.presentation.is_none()
    }

    pub(crate) fn from_runtime(meta: &NodeMeta) -> Self {
        Self {
            decl_id: Some(meta.decl_id.clone()),
            short_name: Some(meta.short_name.clone()),
            enabled: Some(meta.enabled),
            can_be_disabled: Some(meta.can_be_disabled),
            label: Some(meta.label.clone()),
            description: meta.description.clone().map(Some),
            declared_description_key: Some(meta.declared_description_key.clone()),
            declared_description: Some(meta.declared_description.clone()),
            tags: (!meta.tags.is_empty()).then(|| meta.tags.clone()),
            user_permissions: Some(meta.user_permissions.clone()),
            semantics: (meta.semantics != SemanticsHint::default()).then(|| meta.semantics.clone()),
            presentation: (meta.presentation != PresentationHint::default()).then(|| meta.presentation.clone()),
        }
    }

    pub(crate) fn delta_against(&self, baseline: &Self) -> Self {
        Self {
            decl_id: (self.decl_id != baseline.decl_id)
                .then(|| self.decl_id.clone())
                .flatten(),
            short_name: (self.short_name != baseline.short_name)
                .then(|| self.short_name.clone())
                .flatten(),
            enabled: (self.enabled != baseline.enabled).then_some(self.enabled).flatten(),
            can_be_disabled: (self.can_be_disabled != baseline.can_be_disabled)
                .then_some(self.can_be_disabled)
                .flatten(),
            label: (self.label != baseline.label).then(|| self.label.clone()).flatten(),
            description: if self.description != baseline.description {
                self.description.clone()
            } else {
                Default::default()
            },
            declared_description_key: if self.declared_description_key != baseline.declared_description_key {
                self.declared_description_key.clone()
            } else {
                Default::default()
            },
            declared_description: if self.declared_description != baseline.declared_description {
                self.declared_description.clone()
            } else {
                Default::default()
            },
            tags: (self.tags != baseline.tags).then(|| self.tags.clone()).flatten(),
            user_permissions: (self.user_permissions != baseline.user_permissions)
                .then(|| self.user_permissions.clone())
                .flatten(),
            semantics: (self.semantics != baseline.semantics)
                .then(|| self.semantics.clone())
                .flatten(),
            presentation: (self.presentation != baseline.presentation)
                .then(|| self.presentation.clone())
                .flatten(),
        }
    }

    pub(crate) fn without_runtime_fields(&self) -> Self {
        let mut sanitized = self.clone();
        if let Some(presentation) = sanitized.presentation.as_mut() {
            presentation.warnings.clear();
        }
        sanitized
    }

    pub(crate) fn merged_with_sparse_overlay(&self, overlay: &Self) -> Self {
        Self {
            decl_id: overlay.decl_id.clone().or_else(|| self.decl_id.clone()),
            short_name: overlay.short_name.clone().or_else(|| self.short_name.clone()),
            enabled: overlay.enabled.or(self.enabled),
            can_be_disabled: overlay.can_be_disabled.or(self.can_be_disabled),
            label: overlay.label.clone().or_else(|| self.label.clone()),
            description: overlay.description.clone().or_else(|| self.description.clone()),
            declared_description_key: overlay
                .declared_description_key
                .clone()
                .or_else(|| self.declared_description_key.clone()),
            declared_description: overlay
                .declared_description
                .clone()
                .or_else(|| self.declared_description.clone()),
            tags: overlay.tags.clone().or_else(|| self.tags.clone()),
            user_permissions: overlay
                .user_permissions
                .clone()
                .or_else(|| self.user_permissions.clone()),
            semantics: overlay.semantics.clone().or_else(|| self.semantics.clone()),
            presentation: overlay.presentation.clone().or_else(|| self.presentation.clone()),
        }
    }

    pub(crate) fn apply_to_runtime(&self, meta: &mut NodeMeta, uuid: NodeUuid) {
        meta.uuid = uuid;

        if let Some(decl_id) = self.decl_id.clone() {
            meta.decl_id = decl_id;
        }
        if let Some(short_name) = self.short_name.clone() {
            meta.short_name = short_name;
        }
        if let Some(enabled) = self.enabled {
            meta.enabled = enabled;
        }
        if let Some(can_be_disabled) = self.can_be_disabled {
            meta.can_be_disabled = can_be_disabled;
        }
        if let Some(label) = self.label.clone() {
            meta.label = label;
        }
        if let Some(description) = self.description.clone() {
            meta.description = description;
        }
        if let Some(declared_description_key) = self.declared_description_key.clone() {
            meta.declared_description_key = declared_description_key;
        }
        if let Some(declared_description) = self.declared_description.clone() {
            meta.declared_description = declared_description;
        }
        if let Some(tags) = self.tags.clone() {
            meta.tags = tags;
        }
        if let Some(user_permissions) = self.user_permissions.clone() {
            meta.user_permissions = user_permissions;
        } else if meta.user_permissions == NodeUserPermissions::default()
            && self
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag == "is_user_made"))
        {
            // Infer permissions when an older persisted node has only the user-made tag.
            meta.user_permissions = NodeUserPermissions::all();
        }
        if let Some(semantics) = self.semantics.clone() {
            meta.semantics = semantics;
        }
        if let Some(presentation) = self.presentation.clone() {
            meta.presentation = presentation;
        }
    }

    pub(crate) fn into_runtime(self, uuid: NodeUuid) -> NodeMeta {
        let fallback_label = self
            .label
            .clone()
            .or_else(|| self.short_name.clone())
            .unwrap_or_else(|| "Node".to_string());
        let mut meta = NodeMeta::new(fallback_label);
        self.apply_to_runtime(&mut meta, uuid);
        meta
    }
}

impl golden_persistence::ProjectMetadata for ProjectNodeMeta {
    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }
}

/// Error returned by project save/load operations.
#[derive(Debug)]
pub enum ProjectPersistenceError {
    /// I/O failure while reading/writing a project file.
    Io(std::io::Error),
    /// JSON parsing/serialization error.
    Json(serde_json::Error),
    /// Engine structural error while rebuilding the graph.
    Engine(EngineEditError),
    /// Graph traversal encountered a missing node id.
    MissingNode(NodeId),
    /// Unsupported project format version.
    UnsupportedVersion {
        /// Version found in the loaded file.
        found: String,
        /// Expected version supported by this loader.
        expected: &'static str,
    },
    /// Node codec failure for one specific node type.
    Codec {
        /// Node type for which encoding/decoding failed.
        node_type: String,
        /// Human-readable codec message.
        message: String,
    },
    /// Primary and backup project files could not produce a valid document.
    Recovery {
        /// Combined recovery diagnostic.
        message: String,
    },
}

/// Recoverable problems encountered while rebuilding a project file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectLoadRecoveryReport {
    /// Problems that were skipped so the loader could keep the valid graph content.
    pub problems: Vec<ProjectLoadRecoveryProblem>,
}

impl ProjectLoadRecoveryReport {
    /// Returns true when the project loaded without any skipped rebuild work.
    pub fn is_empty(&self) -> bool {
        self.problems.is_empty()
    }

    /// Builds a report for one engine rebuild failure.
    pub fn from_engine_rebuild_error(error: &EngineEditError) -> Self {
        Self {
            problems: vec![ProjectLoadRecoveryProblem {
                stage: ProjectLoadRecoveryStage::LifecycleReplay,
                message: format!("engine rebuild error: {error}"),
            }],
        }
    }

    /// Builds a report for one runtime startup failure.
    pub fn from_runtime_startup_error(message: impl Into<String>) -> Self {
        let mut report = Self::default();
        report.push_runtime_startup_error(message);
        report
    }

    pub(super) fn push_engine_rebuild_error(&mut self, error: EngineEditError) {
        self.problems.push(ProjectLoadRecoveryProblem {
            stage: ProjectLoadRecoveryStage::LifecycleReplay,
            message: format!("engine rebuild error: {error}"),
        });
    }

    /// Records one skipped runtime startup failure.
    pub fn push_runtime_startup_error(&mut self, message: impl Into<String>) {
        self.problems.push(ProjectLoadRecoveryProblem {
            stage: ProjectLoadRecoveryStage::RuntimeStartup,
            message: format!("runtime startup error: {}", message.into()),
        });
    }

    /// Records that the last complete backup replaced an unreadable primary project file.
    pub fn push_project_file_recovery(&mut self, message: impl Into<String>) {
        self.problems.push(ProjectLoadRecoveryProblem {
            stage: ProjectLoadRecoveryStage::ProjectFile,
            message: message.into(),
        });
    }
}

/// One recoverable project-load problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectLoadRecoveryProblem {
    /// Load stage where the problem was detected.
    pub stage: ProjectLoadRecoveryStage,
    /// Human-readable description of the skipped problem.
    pub message: String,
}

/// Project-load stage for a recoverable problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectLoadRecoveryStage {
    /// Primary file corruption or interruption required the last complete backup.
    ProjectFile,
    /// Load-time lifecycle replay emitted invalid rebuild edits.
    LifecycleReplay,
    /// Host runtime startup emitted invalid edits or failed to resolve.
    RuntimeStartup,
}

impl ProjectLoadRecoveryStage {
    /// Stable protocol value for this recovery stage.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectFile => "project_file",
            Self::LifecycleReplay => "lifecycle_replay",
            Self::RuntimeStartup => "runtime_startup",
        }
    }
}

impl fmt::Display for ProjectPersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "project I/O error: {err}"),
            Self::Json(err) => write!(f, "project JSON error: {err}"),
            Self::Engine(err) => write!(f, "engine rebuild error: {err}"),
            Self::MissingNode(node) => write!(f, "project graph references missing node id {:?}", node),
            Self::UnsupportedVersion { found, expected } => {
                write!(f, "unsupported project version '{found}' (expected '{expected}')")
            }
            Self::Codec { node_type, message } => write!(f, "node codec error for '{node_type}': {message}"),
            Self::Recovery { message } => write!(f, "project recovery error: {message}"),
        }
    }
}

impl std::error::Error for ProjectPersistenceError {}

impl From<std::io::Error> for ProjectPersistenceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ProjectPersistenceError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<golden_persistence::ProjectDocumentCodecError> for ProjectPersistenceError {
    fn from(value: golden_persistence::ProjectDocumentCodecError) -> Self {
        match value {
            golden_persistence::ProjectDocumentCodecError::UnsupportedVersion { found, expected } => {
                Self::UnsupportedVersion { found, expected }
            }
            golden_persistence::ProjectDocumentCodecError::Json(error) => Self::Json(error),
        }
    }
}

impl From<EngineEditError> for ProjectPersistenceError {
    fn from(value: EngineEditError) -> Self {
        Self::Engine(value)
    }
}
