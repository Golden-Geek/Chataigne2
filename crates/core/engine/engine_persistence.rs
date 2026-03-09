use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::node::{DeclId, Node, NodeId, NodeMeta, NodeReference, NodeUserPermissions, NodeUuid, PresentationHint, SemanticsHint, UserNodeRole};

use super::engine_history::AddNodeEffect;
use super::{Engine, EngineEditError};

/// Version tag emitted in project files created by this engine.
pub const PROJECT_FILE_VERSION: &str = "1.0";

fn default_project_file_version() -> String {
    PROJECT_FILE_VERSION.to_string()
}

fn is_default_semantics_hint(value: &SemanticsHint) -> bool {
    *value == SemanticsHint::default()
}

fn is_default_presentation_hint(value: &PresentationHint) -> bool {
    *value == PresentationHint::default()
}

fn is_default_node_user_permissions(value: &NodeUserPermissions) -> bool {
    *value == NodeUserPermissions::default()
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub meta: ProjectNodeMeta,
    /// Node-specific payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Ordered child records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ProjectNodeRecord>,
}

/// Persisted subset of runtime node metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectNodeMeta {
    /// Declared id key under the parent scope.
    pub decl_id: DeclId,
    /// Generated short name.
    pub short_name: String,
    /// Runtime enablement state.
    pub enabled: bool,
    /// Whether this node can be disabled.
    pub can_be_disabled: bool,
    /// User-visible label.
    pub label: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Canonical declaration-description key shared by repeated declared nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description_key: Option<String>,
    /// Canonical declaration description before any instance-level override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description: Option<String>,
    /// User tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// User-edit permissions.
    #[serde(default, skip_serializing_if = "is_default_node_user_permissions")]
    pub user_permissions: NodeUserPermissions,
    /// Semantic hints.
    #[serde(default, skip_serializing_if = "is_default_semantics_hint")]
    pub semantics: SemanticsHint,
    /// Presentation hints.
    #[serde(default, skip_serializing_if = "is_default_presentation_hint")]
    pub presentation: PresentationHint,
}

impl ProjectNodeMeta {
    fn from_runtime(meta: &NodeMeta) -> Self {
        Self {
            decl_id: meta.decl_id.clone(),
            short_name: meta.short_name.clone(),
            enabled: meta.enabled,
            can_be_disabled: meta.can_be_disabled,
            label: meta.label.clone(),
            description: meta.description.clone(),
            declared_description_key: meta.declared_description_key.clone(),
            declared_description: meta.declared_description.clone(),
            tags: meta.tags.clone(),
            user_permissions: meta.user_permissions.clone(),
            semantics: meta.semantics.clone(),
            presentation: meta.presentation.clone(),
        }
    }

    fn into_runtime(self, uuid: NodeUuid) -> NodeMeta {
        let mut user_permissions = self.user_permissions;
        // Backward compatibility for persisted projects that predate explicit permission fields.
        if user_permissions == NodeUserPermissions::default() && self.tags.iter().any(|tag| tag == "is_user_made") {
            user_permissions = NodeUserPermissions::all();
        }

        NodeMeta {
            uuid,
            decl_id: self.decl_id,
            short_name: self.short_name,
            enabled: self.enabled,
            can_be_disabled: self.can_be_disabled,
            label: self.label,
            description: self.description,
            declared_description_key: self.declared_description_key,
            declared_description: self.declared_description,
            tags: self.tags,
            user_permissions,
            semantics: self.semantics,
            presentation: self.presentation,
        }
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
            Self::UnsupportedVersion { found, expected } => write!(f, "unsupported project version '{found}' (expected '{expected}')"),
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
        Ok(ProjectFile { version: PROJECT_FILE_VERSION.to_string(), root })
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
            return Err(ProjectPersistenceError::UnsupportedVersion { found: project.version, expected: PROJECT_FILE_VERSION });
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

        // Freshly loaded projects should start with no pending edits/history/events.
        engine.inbox.clear();
        engine.edits.pending.clear();
        engine.clear_history();
        engine.event_listeners.clear();
        engine.expression_runtime.clear();
        engine.time = super::EngineTime { tick: 0, micro: 0, seq: 0 };

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

    pub(crate) fn duplicate_subtree_with<Encode, Decode>(&mut self, source: NodeId, new_parent: NodeId, new_prev_sibling: Option<NodeId>, label: Option<String>, mut encode_data: Encode, mut decode_node: Decode) -> Result<NodeId, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut record = self.encode_node_record_with(source, &mut encode_data)?;
        if let Some(label) = label {
            let short_name = generate_short_name(&label);
            record.meta.label = label;
            record.meta.short_name = short_name.clone();
            record.meta.decl_id = DeclId(short_name);
        }

        let mut uuid_map = HashMap::<NodeUuid, NodeUuid>::new();
        remap_record_uuids(&mut record, &mut uuid_map);

        let duplicated_root = self.insert_duplicate_record_subtree_with(new_parent, new_prev_sibling, &record, &uuid_map, &mut decode_node)?;

        self.resolve_reference_caches();
        self.sync_missing_reference_warnings_silent();
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        self.push_ui_custom_event("__transport.resync_required", Some(duplicated_root), serde_json::json!({ "reason": "duplicate_subtree_loaded" }));
        self.record_single_history_step(
            AddNodeEffect {
                node: duplicated_root,
                parent: new_parent,
                prev_sibling: self.nodes.get(duplicated_root).and_then(|node| node.node_data().prev_sibling),
                next_sibling: self.nodes.get(duplicated_root).and_then(|node| node.node_data().next_sibling),
            }
            .into(),
        );

        Ok(duplicated_root)
    }

    fn encode_node_record_with<F>(&self, node_id: NodeId, encode_data: &mut F) -> Result<ProjectNodeRecord, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let node = self.nodes.get(node_id).ok_or(ProjectPersistenceError::MissingNode(node_id))?;

        let node_type = node.get_type().to_string();
        let meta = ProjectNodeMeta::from_runtime(&node.node_data().meta);
        let uuid = node.node_data().meta.uuid;

        let data_value = encode_data(node).map_err(|message| ProjectPersistenceError::Codec { node_type: node_type.clone(), message })?;
        let data = (!data_value.is_null()).then_some(data_value);

        let mut child_ids = Vec::new();
        let mut child = node.node_data().first_child;
        while let Some(child_id) = child {
            child_ids.push(child_id);
            child = self.nodes.get(child_id).ok_or(ProjectPersistenceError::MissingNode(child_id))?.node_data().next_sibling;
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

    fn decode_node_record_with<F>(parent: Option<&T>, record: &ProjectNodeRecord, decode_node: &mut F) -> Result<T, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let meta = record.meta.clone().into_runtime(record.uuid);
        let data = record.data.clone().unwrap_or(serde_json::Value::Null);
        let mut node = if record.user_role == UserNodeRole::ItemRoot {
            if let Some(parent) = parent {
                if let Some(mut node) = parent.create_user_item(record.node_type.as_str()) {
                    node.node_data_mut().meta.label = meta.label.clone();
                    node.project_decode_data(&data).map_err(|message| ProjectPersistenceError::Codec { node_type: record.node_type.clone(), message })?;
                    T::from_boxed_node(node).ok_or(ProjectPersistenceError::Codec {
                        node_type: record.node_type.clone(),
                        message: "parent item factory returned a node outside the engine node enum".to_string(),
                    })?
                } else {
                    decode_node(&record.node_type, &data, &meta).map_err(|message| ProjectPersistenceError::Codec { node_type: record.node_type.clone(), message })?
                }
            } else {
                decode_node(&record.node_type, &data, &meta).map_err(|message| ProjectPersistenceError::Codec { node_type: record.node_type.clone(), message })?
            }
        } else {
            decode_node(&record.node_type, &data, &meta).map_err(|message| ProjectPersistenceError::Codec { node_type: record.node_type.clone(), message })?
        };

        let node_data = node.node_data_mut();
        node_data.parent = None;
        node_data.first_child = None;
        node_data.last_child = None;
        node_data.prev_sibling = None;
        node_data.next_sibling = None;
        node_data.user_role = record.user_role;
        node_data.meta = meta;

        Ok(node)
    }

    fn load_children_records<F>(&mut self, parent: NodeId, children: &[ProjectNodeRecord], decode_node: &mut F) -> Result<(), ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut prev_sibling = None;

        for child_record in children {
            let child = {
                let parent_node = self.nodes.get(parent).ok_or(ProjectPersistenceError::MissingNode(parent))?;
                Self::decode_node_record_with(Some(parent_node), child_record, decode_node)?
            };
            let child_id = self.nodes.insert(child);
            self.attach_node(0, "LoadProject", child_id, parent, prev_sibling)?;
            self.load_children_records(child_id, &child_record.children, decode_node)?;
            prev_sibling = Some(child_id);
        }

        Ok(())
    }

    fn insert_duplicate_record_subtree_with<F>(&mut self, parent: NodeId, prev_sibling: Option<NodeId>, record: &ProjectNodeRecord, uuid_map: &HashMap<NodeUuid, NodeUuid>, decode_node: &mut F) -> Result<NodeId, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut node = {
            let parent_node = self.nodes.get(parent).ok_or(ProjectPersistenceError::MissingNode(parent))?;
            Self::decode_node_record_with(Some(parent_node), record, decode_node)?
        };
        remap_node_references(&mut node, uuid_map);

        let node_id = self.nodes.insert(node);
        self.attach_node(0, "DuplicateNode", node_id, parent, prev_sibling)?;

        let mut child_prev_sibling = None;
        for child_record in &record.children {
            let child_id = self.insert_duplicate_record_subtree_with(node_id, child_prev_sibling, child_record, uuid_map, decode_node)?;
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
