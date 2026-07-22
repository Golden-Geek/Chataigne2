use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::{Event, EventFrame, EventKind};
use crate::node::{
    DeclId, Node, NodeCreationContext, NodeId, NodeMeta, NodeReference, NodeUserPermissions, NodeUuid,
    PresentationHint, SemanticsHint, UserNodeRole, user_context_multiplex_list_value_type,
};
use crate::process_ctx::ExecutionPhase;
use crate::ui_sync::UiGraphOp;

use super::history::AddNodeEffect;
use super::{Engine, EngineEditError};

/// Version tag emitted in project files created by this engine.
pub const PROJECT_FILE_VERSION: &str = "1.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadedReadyMode {
    Immediate,
    Deferred,
}

fn default_project_file_version() -> String {
    PROJECT_FILE_VERSION.to_string()
}

fn is_default_user_node_role(value: &UserNodeRole) -> bool {
    *value == UserNodeRole::Regular
}

/// Serialized project document containing one rooted node hierarchy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    /// File format version.
    #[serde(default = "default_project_file_version")]
    pub version: String,
    /// Optional project-owned UI state carried verbatim by hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_state: Option<serde_json::Value>,
    /// Root node record.
    pub root: ProjectNodeRecord,
}

/// Serialized node record for full-snapshot persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectNodeRecord {
    /// Persistent identity.
    pub uuid: NodeUuid,
    /// Runtime node type identifier (`Node::get_type()`).
    #[serde(rename = "type")]
    pub node_type: String,
    /// User-facing curation role for this node.
    #[serde(default, skip_serializing_if = "is_default_user_node_role")]
    pub user_role: UserNodeRole,
    /// Persisted metadata fields.
    #[serde(default, skip_serializing_if = "ProjectNodeMeta::is_empty")]
    pub meta: ProjectNodeMeta,
    /// Node-specific payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Ordered child records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ProjectNodeRecord>,
}

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
            description: (self.description != baseline.description)
                .then(|| self.description.clone())
                .unwrap_or_default(),
            declared_description_key: (self.declared_description_key != baseline.declared_description_key)
                .then(|| self.declared_description_key.clone())
                .unwrap_or_default(),
            declared_description: (self.declared_description != baseline.declared_description)
                .then(|| self.declared_description.clone())
                .unwrap_or_default(),
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

    fn push_engine_rebuild_error(&mut self, error: EngineEditError) {
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

impl From<EngineEditError> for ProjectPersistenceError {
    fn from(value: EngineEditError) -> Self {
        Self::Engine(value)
    }
}

impl<T: Node> Engine<T> {
    /// Builds a full in-memory project snapshot using a node-data encoder callback.
    ///
    /// The callback receives each runtime node and must return a JSON payload
    /// for node-specific fields.
    pub fn to_project_file_with<F>(&self, mut encode_data: F) -> Result<ProjectFile, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let root = self.encode_node_record_with(self.root, &mut encode_data)?;
        Ok(ProjectFile {
            version: PROJECT_FILE_VERSION.to_string(),
            ui_state: None,
            root,
        })
    }

    /// Serializes a project snapshot to compact JSON.
    pub fn to_project_json_with<F>(&self, encode_data: F) -> Result<String, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let project = self.to_project_file_with(encode_data)?;
        Ok(serde_json::to_string(&project)?)
    }

    /// Serializes a project snapshot to pretty-printed JSON.
    pub fn to_project_json_pretty_with<F>(&self, encode_data: F) -> Result<String, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let project = self.to_project_file_with(encode_data)?;
        Ok(serde_json::to_string_pretty(&project)?)
    }

    /// Writes a pretty JSON project snapshot to disk.
    pub fn save_project_file_with<P, F>(&self, path: P, encode_data: F) -> Result<(), ProjectPersistenceError>
    where
        P: AsRef<Path>,
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let json = self.to_project_json_pretty_with(encode_data)?;
        golden_persistence::write_file_atomically_with_recovery(path, json.as_bytes())?;
        Ok(())
    }

    /// Loads a project from an already parsed project document.
    ///
    /// The decoder callback receives `(node_type, data, meta)` and must return a
    /// concrete runtime node instance for each record.
    pub fn from_project_file_with<F>(project: ProjectFile, decode_node: F) -> Result<Self, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        Self::from_project_file_with_recovery_mode(project, decode_node, false).map(|(engine, _)| engine)
    }

    /// Loads a project from an already parsed document, skipping recoverable rebuild problems.
    ///
    /// This is intended for explicit user-approved recovery flows. Parse, version,
    /// and codec errors still fail because no coherent runtime graph is available.
    pub fn from_project_file_with_recovery<F>(
        project: ProjectFile,
        decode_node: F,
    ) -> Result<(Self, ProjectLoadRecoveryReport), ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        Self::from_project_file_with_recovery_mode(project, decode_node, true)
    }

    fn from_project_file_with_recovery_mode<F>(
        project: ProjectFile,
        mut decode_node: F,
        recover: bool,
    ) -> Result<(Self, ProjectLoadRecoveryReport), ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        if project.version != PROJECT_FILE_VERSION {
            return Err(ProjectPersistenceError::UnsupportedVersion {
                found: project.version,
                expected: PROJECT_FILE_VERSION,
            });
        }

        let mut root = Self::decode_node_record_with(None, &project.root, &mut decode_node)?;
        {
            let root_data = root.node_data_mut();
            root_data.parent = None;
            root_data.first_child = None;
            root_data.last_child = None;
            root_data.prev_sibling = None;
            root_data.next_sibling = None;
        }

        let decode_started = std::time::Instant::now();
        let mut engine = Engine::new(root);
        let root_id = engine.root;
        engine.load_children_records(root_id, &project.root.children, &mut decode_node)?;
        let decode_elapsed = decode_started.elapsed();

        // Rebuild runtime caches for UUID-based references before lifecycle hooks run.
        // Presentation warnings are synced once after lifecycle has restored declared structure.
        let caches_started = std::time::Instant::now();
        engine.resolve_reference_caches();
        engine.rebuild_user_context_registry_from_nodes();
        let caches_elapsed = caches_started.elapsed();

        // Freshly loaded projects should start with clean runtime-only state before re-running node lifecycle hooks.
        engine.inbox.clear();
        engine.edits.pending.clear();
        engine.clear_history();
        engine.event_listeners.clear();
        engine.expression_runtime.clear();
        engine.time = super::EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        };

        let mut recovery = ProjectLoadRecoveryReport::default();
        let replay_started = std::time::Instant::now();
        match engine.replay_loaded_subtree_lifecycle(
            root_id,
            NodeCreationContext::ProjectLoad,
            LoadedReadyMode::Deferred,
        ) {
            Ok(()) => {}
            Err(ProjectPersistenceError::Engine(error)) if recover => {
                recovery.push_engine_rebuild_error(error);
                engine.inbox.clear();
                engine.edits.pending.clear();
                engine.pending_node_ready.clear();
                engine.clear_history();
            }
            Err(error) => return Err(error),
        }
        let replay_elapsed = replay_started.elapsed();
        let warnings_started = std::time::Instant::now();
        engine.sync_missing_reference_warnings_silent();
        engine.rebuild_user_context_registry_from_nodes();
        eprintln!(
            "[project-load] decode_ms={} caches_ms={} replay_ms={} warnings_ms={}",
            decode_elapsed.as_millis(),
            caches_elapsed.as_millis(),
            replay_elapsed.as_millis(),
            warnings_started.elapsed().as_millis()
        );

        // Loaded projects should not surface bootstrap-only events or history to the runtime host.
        engine.inbox.clear();
        engine.edits.pending.clear();
        engine.clear_history();

        Ok((engine, recovery))
    }

    /// Loads a project from a JSON string.
    pub fn from_project_json_with<F>(json: &str, decode_node: F) -> Result<Self, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let project: ProjectFile = serde_json::from_str(json)?;
        Self::from_project_file_with(project, decode_node)
    }

    /// Loads a project from a file path.
    pub fn load_project_file_with<P, F>(path: P, decode_node: F) -> Result<Self, ProjectPersistenceError>
    where
        P: AsRef<Path>,
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let json = fs::read_to_string(path)?;
        Self::from_project_json_with(&json, decode_node)
    }

    /// Duplicates a persisted subtree by round-tripping through the project codec hooks.
    pub fn duplicate_subtree_with<Encode, Decode>(
        &mut self,
        source: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
        label_override: Option<String>,
        encode_data: Encode,
        decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        self.duplicate_subtree_with_dispatch(
            source,
            new_parent,
            new_prev_sibling,
            label_override,
            true,
            encode_data,
            decode_node,
        )
    }

    pub(crate) fn duplicate_subtree_with_dispatch<Encode, Decode>(
        &mut self,
        source: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
        label_override: Option<String>,
        dispatch_structure_events: bool,
        mut encode_data: Encode,
        mut decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut record = self.encode_node_record_with(source, &mut encode_data)?;
        let source_label = record
            .meta
            .label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(record.node_type.as_str());
        let preferred_label = label_override
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(source_label);
        let label = self.next_unique_child_label(new_parent, preferred_label);
        let short_name = generate_short_name(&label);
        record.meta.label = Some(label);
        record.meta.short_name = Some(short_name.clone());
        record.meta.decl_id = Some(DeclId(short_name));

        let mut uuid_map = HashMap::<NodeUuid, NodeUuid>::new();
        remap_record_uuids(&mut record, &mut uuid_map);

        let duplicated_root = self.insert_duplicate_record_subtree_with(
            new_parent,
            new_prev_sibling,
            &record,
            &uuid_map,
            &mut decode_node,
        )?;
        self.replay_loaded_subtree_lifecycle(
            duplicated_root,
            NodeCreationContext::Duplicate,
            LoadedReadyMode::Immediate,
        )?;
        let duplicated_node_ids = self.collect_loaded_subtree_node_ids(duplicated_root)?;
        if dispatch_structure_events {
            self.queue_loaded_subtree_structure_events(&[duplicated_root])?;
        }
        self.sync_missing_reference_warnings_for_nodes_silent(duplicated_node_ids.as_slice());
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        self.push_loaded_subtree_ui_events(duplicated_node_ids.as_slice())?;
        self.record_single_history_step(
            AddNodeEffect {
                node: duplicated_root,
                parent: new_parent,
                prev_sibling: self
                    .nodes
                    .get(duplicated_root)
                    .and_then(|node| node.node_data().prev_sibling),
                next_sibling: self
                    .nodes
                    .get(duplicated_root)
                    .and_then(|node| node.node_data().next_sibling),
            }
            .into(),
        );

        Ok(duplicated_root)
    }

    /// Inserts one persisted node hierarchy beneath an existing parent.
    ///
    /// Imported UUIDs are remapped as one unit, so internal references remain
    /// valid without colliding with nodes already present in the engine.
    pub fn insert_project_subtree_with<Decode>(
        &mut self,
        project: ProjectFile,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        mut decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        if project.version != PROJECT_FILE_VERSION {
            return Err(ProjectPersistenceError::UnsupportedVersion {
                found: project.version,
                expected: PROJECT_FILE_VERSION,
            });
        }
        if !self.nodes.contains(parent) {
            return Err(ProjectPersistenceError::MissingNode(parent));
        }

        let mut record = project.root;
        let mut uuid_map = HashMap::<NodeUuid, NodeUuid>::new();
        remap_record_uuids(&mut record, &mut uuid_map);
        let imported_root =
            self.insert_duplicate_record_subtree_with(parent, prev_sibling, &record, &uuid_map, &mut decode_node)?;
        self.replay_loaded_subtree_lifecycle(
            imported_root,
            NodeCreationContext::Duplicate,
            LoadedReadyMode::Immediate,
        )?;
        let imported_node_ids = self.collect_loaded_subtree_node_ids(imported_root)?;
        self.sync_missing_reference_warnings_for_nodes_silent(imported_node_ids.as_slice());
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        self.push_loaded_subtree_ui_events(imported_node_ids.as_slice())?;
        self.record_single_history_step(
            AddNodeEffect {
                node: imported_root,
                parent,
                prev_sibling: self
                    .nodes
                    .get(imported_root)
                    .and_then(|node| node.node_data().prev_sibling),
                next_sibling: self
                    .nodes
                    .get(imported_root)
                    .and_then(|node| node.node_data().next_sibling),
            }
            .into(),
        );

        Ok(imported_root)
    }

    fn push_loaded_subtree_ui_events(&mut self, node_ids: &[NodeId]) -> Result<(), ProjectPersistenceError> {
        let mut ops = Vec::<UiGraphOp>::with_capacity(node_ids.len().saturating_add(1));
        let mut root_parent = None;
        let catalog_snapshot = self.build_process_tree_snapshot();

        // Match the normal add-node path: large loaded/duplicated subtrees should
        // reach the UI as one transaction instead of many indexed inserts.
        const SUBTREE_COMPACT_THRESHOLD: usize = 8;
        if node_ids.len() > SUBTREE_COMPACT_THRESHOLD {
            let Some(root) = node_ids.first().copied() else {
                return Ok(());
            };
            let parent = self
                .nodes
                .get(root)
                .ok_or(ProjectPersistenceError::MissingNode(root))?
                .node_data()
                .parent;
            if let Some(parent) = parent {
                let mut nodes = Vec::with_capacity(node_ids.len());
                for node in node_ids {
                    let snapshot = self
                        .ui_node_dto_for_event_with_catalog_snapshot(*node, catalog_snapshot.as_ref())
                        .ok_or(ProjectPersistenceError::MissingNode(*node))?;
                    nodes.push(snapshot);
                }
                let parent_children_after = self.ui_direct_children(parent).unwrap_or_default();
                self.push_ui_graph_transaction(vec![UiGraphOp::SubtreeInserted {
                    root,
                    parent,
                    nodes,
                    parent_children_after,
                }]);
                return Ok(());
            }
        }

        for node in node_ids {
            let parent = {
                let node_data = self
                    .nodes
                    .get(*node)
                    .ok_or(ProjectPersistenceError::MissingNode(*node))?
                    .node_data();
                node_data.parent
            };
            if root_parent.is_none() {
                root_parent = parent;
            }

            let snapshot = self
                .ui_node_dto_for_event_with_catalog_snapshot(*node, catalog_snapshot.as_ref())
                .ok_or(ProjectPersistenceError::MissingNode(*node))?;
            let index = parent.and_then(|parent| self.ui_child_index(parent, *node));
            ops.push(UiGraphOp::NodeCreated {
                snapshot,
                parent,
                index,
            });
        }

        if let Some(parent) = root_parent {
            if let Some(children) = self.ui_direct_children(parent) {
                ops.push(UiGraphOp::ChildrenReordered { parent, children });
            }
        }

        self.push_ui_graph_transaction(ops);

        Ok(())
    }

    fn replay_loaded_subtree_lifecycle(
        &mut self,
        root: NodeId,
        creation_context: NodeCreationContext,
        ready_mode: LoadedReadyMode,
    ) -> Result<(), ProjectPersistenceError> {
        let mut loaded_node_ids = self.collect_loaded_subtree_node_ids(root)?;

        self.prune_loaded_duplicate_declared_children(loaded_node_ids.as_slice())?;
        loaded_node_ids = self.collect_loaded_subtree_node_ids(root)?;

        self.run_node_attached_for_batch(loaded_node_ids.as_slice(), Some(creation_context))?;
        self.reconcile_loaded_declared_children(loaded_node_ids.as_slice(), creation_context)?;
        loaded_node_ids = self.collect_loaded_subtree_node_ids(root)?;
        self.prune_loaded_duplicate_declared_children(loaded_node_ids.as_slice())?;
        loaded_node_ids = self.collect_loaded_subtree_node_ids(root)?;
        self.reconcile_loaded_declared_children(loaded_node_ids.as_slice(), creation_context)?;
        loaded_node_ids = self.collect_loaded_subtree_node_ids(root)?;
        self.run_node_init_for_batch(loaded_node_ids.as_slice(), Some(creation_context))?;

        match ready_mode {
            LoadedReadyMode::Immediate => {
                self.run_node_ready_for_batch(loaded_node_ids.as_slice(), creation_context)?
            }
            LoadedReadyMode::Deferred => {
                for node_id in loaded_node_ids {
                    self.queue_node_ready(node_id, creation_context);
                }
            }
        }

        Ok(())
    }

    fn prune_loaded_duplicate_declared_children(
        &mut self,
        parent_ids: &[NodeId],
    ) -> Result<(), ProjectPersistenceError> {
        let mut duplicates = Vec::<NodeId>::new();

        for parent_id in parent_ids {
            if !self.nodes.contains(*parent_id) {
                continue;
            }

            let parent_first_child = {
                let parent_node = self
                    .nodes
                    .get(*parent_id)
                    .ok_or(ProjectPersistenceError::MissingNode(*parent_id))?;
                if user_context_multiplex_list_value_type(parent_node.get_type()).is_some() {
                    continue;
                }
                parent_node.node_data().first_child
            };

            let mut seen = HashMap::<String, NodeId>::new();
            let mut child = parent_first_child;

            while let Some(child_id) = child {
                let child_node = self
                    .nodes
                    .get(child_id)
                    .ok_or(ProjectPersistenceError::MissingNode(child_id))?;
                child = child_node.node_data().next_sibling;

                let child_data = child_node.node_data();
                if child_data.user_role != UserNodeRole::Regular {
                    continue;
                }

                let decl_id = child_data.meta.decl_id.0.trim();
                if decl_id.is_empty() {
                    continue;
                }

                let key = format!("{}|{:?}|{}", child_node.get_type(), child_data.user_role, decl_id);
                if seen.insert(key, child_id).is_some() {
                    duplicates.push(child_id);
                }
            }
        }

        for duplicate in duplicates {
            if self.nodes.contains(duplicate) {
                self.discard_loaded_duplicate_declared_subtree(duplicate)?;
            }
        }

        Ok(())
    }

    fn discard_loaded_duplicate_declared_subtree(&mut self, root: NodeId) -> Result<(), ProjectPersistenceError> {
        const OP: &str = "LoadProjectPruneDuplicateDeclaredChild";

        let subtree = self
            .collect_subtree(0, OP, root)
            .map_err(ProjectPersistenceError::Engine)?;
        self.detach_node(0, OP, root).map_err(ProjectPersistenceError::Engine)?;

        for removed in subtree.into_iter().rev() {
            self.unregister_node_uuid(removed);
            self.nodes
                .detach(removed)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OP,
                    node: removed,
                })
                .map_err(ProjectPersistenceError::Engine)?;
            self.purge_param_cache_entry(removed);
            self.blueprints.unregister_instance(removed);
        }

        Ok(())
    }

    pub(crate) fn queue_loaded_subtree_structure_events(
        &mut self,
        node_ids: &[NodeId],
    ) -> Result<(), ProjectPersistenceError> {
        for node_id in node_ids.iter().copied() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let Some(parent) = node.node_data().parent else {
                continue;
            };
            let decl_id = node.node_data().meta.decl_id.clone();

            self.emit_inbox_event(EventKind::NodeCreated { node: node_id });
            self.emit_inbox_event(EventKind::ChildAdded {
                parent,
                child: node_id,
                decl_id,
            });
        }

        Ok(())
    }

    fn reconcile_loaded_declared_children(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), ProjectPersistenceError> {
        let mut index_by_node = HashMap::<NodeId, usize>::new();
        let mut per_node_events = Vec::<(NodeId, EventFrame)>::new();

        for parent_id in node_ids {
            let mut child = self
                .nodes
                .get(*parent_id)
                .ok_or(ProjectPersistenceError::MissingNode(*parent_id))?
                .node_data()
                .first_child;

            while let Some(child_id) = child {
                let child_node = self
                    .nodes
                    .get(child_id)
                    .ok_or(ProjectPersistenceError::MissingNode(child_id))?;
                let event = Event {
                    time: self.time,
                    kind: EventKind::ChildAdded {
                        parent: *parent_id,
                        child: child_id,
                        decl_id: child_node.node_data().meta.decl_id.clone(),
                    },
                };

                let event = Arc::new(event);
                for recipient in self.route_event_recipients(event.as_ref()) {
                    let slot = match index_by_node.get(&recipient).copied() {
                        Some(index) => index,
                        None => {
                            let index = per_node_events.len();
                            per_node_events.push((recipient, EventFrame::new()));
                            index_by_node.insert(recipient, index);
                            index
                        }
                    };
                    per_node_events[slot].1.push_shared(Arc::clone(&event));
                }

                child = child_node.node_data().next_sibling;
            }
        }

        if per_node_events.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        self.preprocess_precomputed_inbox(ExecutionPhase::EngineTick, per_node_events)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
        }

        Ok(())
    }

    fn collect_loaded_subtree_node_ids(&self, root: NodeId) -> Result<Vec<NodeId>, ProjectPersistenceError> {
        if !self.nodes.contains(root) {
            return Err(ProjectPersistenceError::MissingNode(root));
        }

        let mut ordered = Vec::<NodeId>::new();
        let mut stack = vec![root];
        while let Some(node_id) = stack.pop() {
            ordered.push(node_id);

            let mut children = Vec::<NodeId>::new();
            let mut child = self
                .nodes
                .get(node_id)
                .ok_or(ProjectPersistenceError::MissingNode(node_id))?
                .node_data()
                .first_child;
            while let Some(child_id) = child {
                children.push(child_id);
                child = self
                    .nodes
                    .get(child_id)
                    .ok_or(ProjectPersistenceError::MissingNode(child_id))?
                    .node_data()
                    .next_sibling;
            }

            for child_id in children.into_iter().rev() {
                stack.push(child_id);
            }
        }

        Ok(ordered)
    }

    fn encode_node_record_with<F>(
        &self,
        node_id: NodeId,
        encode_data: &mut F,
    ) -> Result<ProjectNodeRecord, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let node = self
            .nodes
            .get(node_id)
            .ok_or(ProjectPersistenceError::MissingNode(node_id))?;

        let node_type = node.get_type().to_string();
        let meta = ProjectNodeMeta::from_runtime(&node.node_data().meta);
        let uuid = node.node_data().meta.uuid;

        let data_value = encode_data(node).map_err(|message| ProjectPersistenceError::Codec {
            node_type: node_type.clone(),
            message,
        })?;
        let data = (!data_value.is_null()).then_some(data_value);

        let mut child_ids = Vec::new();
        let mut child = node.node_data().first_child;
        while let Some(child_id) = child {
            child_ids.push(child_id);
            child = self
                .nodes
                .get(child_id)
                .ok_or(ProjectPersistenceError::MissingNode(child_id))?
                .node_data()
                .next_sibling;
        }

        let mut children = Vec::with_capacity(child_ids.len());
        for child_id in child_ids {
            children.push(self.encode_node_record_with(child_id, encode_data)?);
        }

        Ok(ProjectNodeRecord {
            uuid,
            node_type,
            user_role: node.node_data().user_role,
            meta,
            data,
            children,
        })
    }

    fn decode_node_record_with<F>(
        parent: Option<&T>,
        record: &ProjectNodeRecord,
        decode_node: &mut F,
    ) -> Result<T, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let data = record.data.clone().unwrap_or(serde_json::Value::Null);
        let mut node = if record.user_role == UserNodeRole::ItemRoot {
            if let Some(parent) = parent {
                if let Some(mut node) = parent.create_user_item(record.node_type.as_str()) {
                    node.project_decode_data(&data)
                        .map_err(|message| ProjectPersistenceError::Codec {
                            node_type: record.node_type.clone(),
                            message,
                        })?;
                    T::from_boxed_node(node).ok_or(ProjectPersistenceError::Codec {
                        node_type: record.node_type.clone(),
                        message: "parent item factory returned a node outside the engine node enum".to_string(),
                    })?
                } else {
                    if let Some(mut node) = T::project_create(record.node_type.as_str()) {
                        node.project_decode_data(&data)
                            .map_err(|message| ProjectPersistenceError::Codec {
                                node_type: record.node_type.clone(),
                                message,
                            })?;
                        node
                    } else {
                        let meta = record.meta.clone().into_runtime(record.uuid);
                        decode_node(&record.node_type, &data, &meta).map_err(|message| {
                            ProjectPersistenceError::Codec {
                                node_type: record.node_type.clone(),
                                message,
                            }
                        })?
                    }
                }
            } else {
                if let Some(mut node) = T::project_create(record.node_type.as_str()) {
                    node.project_decode_data(&data)
                        .map_err(|message| ProjectPersistenceError::Codec {
                            node_type: record.node_type.clone(),
                            message,
                        })?;
                    node
                } else {
                    let meta = record.meta.clone().into_runtime(record.uuid);
                    decode_node(&record.node_type, &data, &meta).map_err(|message| ProjectPersistenceError::Codec {
                        node_type: record.node_type.clone(),
                        message,
                    })?
                }
            }
        } else {
            if let Some(mut node) = T::project_create(record.node_type.as_str()) {
                node.project_decode_data(&data)
                    .map_err(|message| ProjectPersistenceError::Codec {
                        node_type: record.node_type.clone(),
                        message,
                    })?;
                node
            } else {
                let meta = record.meta.clone().into_runtime(record.uuid);
                decode_node(&record.node_type, &data, &meta).map_err(|message| ProjectPersistenceError::Codec {
                    node_type: record.node_type.clone(),
                    message,
                })?
            }
        };

        let node_data = node.node_data_mut();
        node_data.parent = None;
        node_data.first_child = None;
        node_data.last_child = None;
        node_data.prev_sibling = None;
        node_data.next_sibling = None;
        node_data.user_role = record.user_role;
        record.meta.apply_to_runtime(&mut node_data.meta, record.uuid);

        Ok(node)
    }

    fn load_children_records<F>(
        &mut self,
        parent: NodeId,
        children: &[ProjectNodeRecord],
        decode_node: &mut F,
    ) -> Result<(), ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut prev_sibling = None;

        for child_record in children {
            let child = {
                let parent_node = self
                    .nodes
                    .get(parent)
                    .ok_or(ProjectPersistenceError::MissingNode(parent))?;
                Self::decode_node_record_with(Some(parent_node), child_record, decode_node)?
            };
            let child_id = self.nodes.insert(child);
            self.register_node_uuid(child_id);
            self.attach_node(0, "LoadProject", child_id, parent, prev_sibling)?;
            self.populate_param_cache_entry(child_id);
            self.load_children_records(child_id, &child_record.children, decode_node)?;
            prev_sibling = Some(child_id);
        }

        Ok(())
    }

    fn insert_duplicate_record_subtree_with<F>(
        &mut self,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        record: &ProjectNodeRecord,
        uuid_map: &HashMap<NodeUuid, NodeUuid>,
        decode_node: &mut F,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut node = {
            let parent_node = self
                .nodes
                .get(parent)
                .ok_or(ProjectPersistenceError::MissingNode(parent))?;
            Self::decode_node_record_with(Some(parent_node), record, decode_node)?
        };
        remap_node_references(&mut node, uuid_map);

        let node_id = self.nodes.insert(node);
        self.register_node_uuid(node_id);
        self.attach_node(0, "DuplicateNode", node_id, parent, prev_sibling)?;
        self.populate_param_cache_entry(node_id);

        let mut child_prev_sibling = None;
        for child_record in &record.children {
            let child_id = self.insert_duplicate_record_subtree_with(
                node_id,
                child_prev_sibling,
                child_record,
                uuid_map,
                decode_node,
            )?;
            child_prev_sibling = Some(child_id);
        }

        Ok(node_id)
    }
}

pub(crate) fn remap_record_uuids(record: &mut ProjectNodeRecord, uuid_map: &mut HashMap<NodeUuid, NodeUuid>) {
    let next_uuid = NodeUuid(Uuid::new_v4());
    uuid_map.insert(record.uuid, next_uuid);
    record.uuid = next_uuid;

    for child in &mut record.children {
        remap_record_uuids(child, uuid_map);
    }
}

fn remap_node_references<T: Node>(node: &mut T, uuid_map: &HashMap<NodeUuid, NodeUuid>) {
    node.engine_visit_references_mut(&mut |reference: &mut NodeReference| {
        if let Some(remapped_uuid) = uuid_map.get(&reference.uuid()) {
            reference.uuid = *remapped_uuid;
        }
        reference.clear_cached_id();
        reference.clear_cached_name();
    });
}

fn generate_short_name(label: &str) -> String {
    let mut short_name = String::new();
    let mut capitalize_next = false;

    for c in label.chars() {
        if c.is_alphanumeric() {
            if capitalize_next {
                short_name.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                short_name.push(c.to_ascii_lowercase());
            }
        } else if c == '+' {
            short_name.push_str(if capitalize_next { "Plus" } else { "plus" });
            capitalize_next = false;
        } else if c == '-' {
            short_name.push_str(if capitalize_next { "Minus" } else { "minus" });
            capitalize_next = false;
        } else {
            capitalize_next = true;
        }
    }

    short_name
}

#[cfg(test)]
mod tests;
