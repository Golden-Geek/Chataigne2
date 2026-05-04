use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::{Event, EventKind};
use crate::node::{
    DeclId, Node, NodeCreationContext, NodeId, NodeMeta, NodeReference, NodeUserPermissions, NodeUuid,
    PresentationHint, SemanticsHint, UserNodeRole,
};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

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
            description: Some(meta.description.clone()),
            declared_description_key: Some(meta.declared_description_key.clone()),
            declared_description: Some(meta.declared_description.clone()),
            tags: Some(meta.tags.clone()),
            user_permissions: Some(meta.user_permissions.clone()),
            semantics: Some(meta.semantics.clone()),
            presentation: Some(meta.presentation.clone()),
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
            // Backward compatibility for persisted projects that predate explicit permission fields.
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
        fs::write(path, json)?;
        Ok(())
    }

    /// Loads a project from an already parsed project document.
    ///
    /// The decoder callback receives `(node_type, data, meta)` and must return a
    /// concrete runtime node instance for each record.
    pub fn from_project_file_with<F>(project: ProjectFile, mut decode_node: F) -> Result<Self, ProjectPersistenceError>
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

        let mut engine = Engine::new(root);
        let root_id = engine.root;
        engine.load_children_records(root_id, &project.root.children, &mut decode_node)?;

        // Rebuild runtime caches for UUID-based references after full tree reconstruction.
        engine.resolve_reference_caches();
        engine.sync_missing_reference_warnings_silent();
        engine.rebuild_user_context_registry_from_nodes();

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

        engine.replay_loaded_subtree_lifecycle(root_id, NodeCreationContext::ProjectLoad, LoadedReadyMode::Deferred)?;
        engine.resolve_reference_caches();
        engine.sync_missing_reference_warnings_silent();
        engine.rebuild_user_context_registry_from_nodes();

        // Loaded projects should not surface bootstrap-only events or history to the runtime host.
        engine.inbox.clear();
        engine.edits.pending.clear();
        engine.clear_history();

        Ok(engine)
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
        label: Option<String>,
        mut encode_data: Encode,
        mut decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut record = self.encode_node_record_with(source, &mut encode_data)?;
        if let Some(label) = label {
            let short_name = generate_short_name(&label);
            record.meta.label = Some(label);
            record.meta.short_name = Some(short_name.clone());
            record.meta.decl_id = Some(DeclId(short_name));
        }

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
        self.resolve_reference_caches();
        self.sync_missing_reference_warnings_silent();
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        self.push_ui_custom_event(
            "__transport.resync_required",
            Some(duplicated_root),
            serde_json::json!({ "reason": "duplicate_subtree_loaded" }),
        );
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

    fn replay_loaded_subtree_lifecycle(
        &mut self,
        root: NodeId,
        creation_context: NodeCreationContext,
        ready_mode: LoadedReadyMode,
    ) -> Result<(), ProjectPersistenceError> {
        let loaded_node_ids = self.collect_loaded_subtree_node_ids(root)?;

        for node_id in &loaded_node_ids {
            self.replay_loaded_node_attached(*node_id, creation_context)?;
        }

        self.reconcile_loaded_declared_children(loaded_node_ids.as_slice(), creation_context)?;

        for node_id in loaded_node_ids {
            self.replay_loaded_node_init(node_id, creation_context)?;
            match ready_mode {
                LoadedReadyMode::Immediate => self.replay_loaded_node_ready(node_id, creation_context)?,
                LoadedReadyMode::Deferred => self.queue_node_ready(node_id, creation_context),
            }
        }

        Ok(())
    }

    fn reconcile_loaded_declared_children(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), ProjectPersistenceError> {
        let mut index_by_node = HashMap::<NodeId, usize>::new();
        let mut per_node_events = Vec::<(NodeId, Vec<Event>)>::new();

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

                for recipient in self.route_event_recipients(&event) {
                    let slot = match index_by_node.get(&recipient).copied() {
                        Some(index) => index,
                        None => {
                            let index = per_node_events.len();
                            per_node_events.push((recipient, Vec::new()));
                            index_by_node.insert(recipient, index);
                            index
                        }
                    };
                    per_node_events[slot].1.push(event.clone());
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

    fn replay_loaded_node_attached(
        &mut self,
        node_id: NodeId,
        creation_context: NodeCreationContext,
    ) -> Result<(), ProjectPersistenceError> {
        let tree_snapshot = Some(self.build_process_tree_snapshot());
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(tree_snapshot) = &tree_snapshot {
            ctx.set_tree_snapshot(Arc::clone(tree_snapshot));
        }

        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.engine_on_attached(&mut ctx);
            });
        }

        let event_cursor = self.inbox.events.len();
        self.absorb_edits(&mut ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
        }

        Ok(())
    }

    fn replay_loaded_node_init(
        &mut self,
        node_id: NodeId,
        creation_context: NodeCreationContext,
    ) -> Result<(), ProjectPersistenceError> {
        let tree_snapshot = Some(self.build_process_tree_snapshot());
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(tree_snapshot) = &tree_snapshot {
            ctx.set_tree_snapshot(Arc::clone(tree_snapshot));
        }

        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.init(&mut ctx);
            });
        }

        let event_cursor = self.inbox.events.len();
        self.absorb_edits(&mut ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
        }

        Ok(())
    }

    fn replay_loaded_node_ready(
        &mut self,
        node_id: NodeId,
        creation_context: NodeCreationContext,
    ) -> Result<(), ProjectPersistenceError> {
        self.run_node_ready(node_id, creation_context)?;
        Ok(())
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
            self.attach_node(0, "LoadProject", child_id, parent, prev_sibling)?;
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
        self.attach_node(0, "DuplicateNode", node_id, parent, prev_sibling)?;

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

fn remap_record_uuids(record: &mut ProjectNodeRecord, uuid_map: &mut HashMap<NodeUuid, NodeUuid>) {
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
mod persistence_tests;
