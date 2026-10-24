use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use golden_persistence::PROJECT_FILE_VERSION;

use crate::events::{Event, EventFrame, EventKind};
use crate::node::{
    DeclId, Node, NodeCreationContext, NodeId, NodeMeta, NodeReference, NodeUserPermissions, NodeUuid,
    PresentationHint, SemanticsHint, UserNodeRole, user_context_multiplex_list_value_type,
};
use crate::process_ctx::{ExecutionPhase, ProcessTreeSnapshot};
use crate::ui_sync::UiGraphOp;

use super::history::AddNodeEffect;
use super::{Engine, EngineEditError};

mod duplicate;
mod import;
mod lifecycle;
mod transaction;
mod types;

use duplicate::DecodedProjectTree;
pub use import::{ProjectImportResult, ProjectImportSpec, ProjectImportedIdentity};
pub use types::{
    ProjectLoadRecoveryProblem, ProjectLoadRecoveryReport, ProjectLoadRecoveryStage, ProjectNodeMeta,
    ProjectPersistenceError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadedReadyMode {
    Immediate,
    Deferred,
}

/// Serialized project document containing one rooted node hierarchy.
pub type ProjectFile = golden_persistence::ProjectDocument<ProjectNodeMeta>;

/// Serialized node record for full-snapshot persistence.
pub type ProjectNodeRecord = golden_persistence::ProjectNodeRecord<ProjectNodeMeta>;

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
        Ok(golden_persistence::encode_project_document_compact(&project)?)
    }

    /// Serializes a project snapshot to pretty-printed JSON.
    pub fn to_project_json_pretty_with<F>(&self, encode_data: F) -> Result<String, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        let project = self.to_project_file_with(encode_data)?;
        Ok(golden_persistence::encode_project_document(&project)?)
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
        golden_persistence::validate_project_document_version(&project)?;

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
        let project = golden_persistence::decode_project_document(json)?;
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
        golden_persistence::validate_project_document_version(&project)?;
        self.validate_persisted_subtree_destination(parent, prev_sibling, "InsertProjectSubtree")?;

        let mut record = project.root;
        let mut uuid_map = HashMap::<NodeUuid, NodeUuid>::new();
        remap_record_uuids(&mut record, &mut uuid_map);
        let decoded_tree = {
            let parent_node = self
                .nodes
                .get(parent)
                .ok_or(ProjectPersistenceError::MissingNode(parent))?;
            Self::decode_project_record_tree_with(parent_node, &record, &uuid_map, &mut decode_node)?
        };
        drop(record);
        drop(uuid_map);
        let commit_checkpoint = self.edit_checkpoint();
        let imported_root =
            match self.insert_decoded_project_tree(parent, prev_sibling, decoded_tree, "InsertProjectSubtree") {
                Ok(root) => root,
                Err(error) => return Err(self.abort_project_subtree_commit(commit_checkpoint, error)),
            };
        if let Err(error) = self.replay_loaded_subtree_lifecycle(
            imported_root,
            NodeCreationContext::Duplicate,
            LoadedReadyMode::Immediate,
        ) {
            return Err(self.abort_project_subtree_commit(commit_checkpoint, error));
        }
        let imported_node_ids = match self.collect_loaded_subtree_node_ids(imported_root) {
            Ok(node_ids) => node_ids,
            Err(error) => {
                return Err(self.abort_project_subtree_commit(commit_checkpoint, error));
            }
        };
        self.sync_missing_reference_warnings_for_nodes_silent(imported_node_ids.as_slice());
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        if let Err(error) = self.push_loaded_subtree_ui_events(imported_node_ids.as_slice()) {
            return Err(self.abort_project_subtree_commit(commit_checkpoint, error));
        }
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

        self.commit_edit_checkpoint(commit_checkpoint);
        Ok(imported_root)
    }

    /// Restores one persisted node hierarchy with its archived UUIDs intact.
    ///
    /// Unlike [`Self::insert_project_subtree_with`], this operation is for a
    /// subtree that previously belonged to the same project and was removed
    /// before restoration. Every archived UUID is reserved up front and the
    /// operation fails before mutation when any identity is already live.
    pub fn restore_project_subtree_with<Decode>(
        &mut self,
        project: ProjectFile,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        mut decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        golden_persistence::validate_project_document_version(&project)?;
        self.validate_persisted_subtree_destination(parent, prev_sibling, "RestoreProjectSubtree")?;

        let mut archived = HashSet::new();
        collect_record_uuids(&project.root, &mut archived)?;
        if let Some(uuid) = archived.iter().find(|uuid| self.uuid_index.contains_key(uuid)).copied() {
            return Err(ProjectPersistenceError::Codec {
                node_type: project.root.node_type.clone(),
                message: format!(
                    "cannot restore archived subtree because UUID {} is already live",
                    uuid.0
                ),
            });
        }

        let decoded_tree = {
            let parent_node = self
                .nodes
                .get(parent)
                .ok_or(ProjectPersistenceError::MissingNode(parent))?;
            Self::decode_project_record_tree_with(parent_node, &project.root, &HashMap::new(), &mut decode_node)?
        };
        let commit_checkpoint = self.edit_checkpoint();
        let restored_root =
            match self.insert_decoded_project_tree(parent, prev_sibling, decoded_tree, "RestoreProjectSubtree") {
                Ok(root) => root,
                Err(error) => return Err(self.abort_project_subtree_commit(commit_checkpoint, error)),
            };
        if let Err(error) = self.replay_loaded_subtree_lifecycle(
            restored_root,
            NodeCreationContext::ProjectLoad,
            LoadedReadyMode::Immediate,
        ) {
            return Err(self.abort_project_subtree_commit(commit_checkpoint, error));
        }
        let restored_node_ids = match self.collect_loaded_subtree_node_ids(restored_root) {
            Ok(node_ids) => node_ids,
            Err(error) => {
                return Err(self.abort_project_subtree_commit(commit_checkpoint, error));
            }
        };
        self.sync_missing_reference_warnings_for_nodes_silent(restored_node_ids.as_slice());
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        if let Err(error) = self.push_loaded_subtree_ui_events(restored_node_ids.as_slice()) {
            return Err(self.abort_project_subtree_commit(commit_checkpoint, error));
        }
        self.record_single_history_step(
            AddNodeEffect {
                node: restored_root,
                parent,
                prev_sibling: self
                    .nodes
                    .get(restored_root)
                    .and_then(|node| node.node_data().prev_sibling),
                next_sibling: self
                    .nodes
                    .get(restored_root)
                    .and_then(|node| node.node_data().next_sibling),
            }
            .into(),
        );
        self.commit_edit_checkpoint(commit_checkpoint);
        Ok(restored_root)
    }

    /// Restores several archived sibling subtrees as one all-or-nothing batch.
    ///
    /// Every document is validated and decoded, and every UUID is reserved,
    /// before the first live node is inserted. Lifecycle replay and UI
    /// publication observe the complete forest; any failure rolls the whole
    /// batch back to its pre-commit checkpoint.
    pub fn restore_project_subtrees_with<Decode>(
        &mut self,
        projects: Vec<ProjectFile>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        mut decode_node: Decode,
    ) -> Result<Vec<NodeId>, ProjectPersistenceError>
    where
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        self.validate_persisted_subtree_destination(parent, prev_sibling, "RestoreProjectSubtrees")?;
        let mut archived = HashSet::new();
        for project in &projects {
            golden_persistence::validate_project_document_version(project)?;
            collect_record_uuids(&project.root, &mut archived)?;
        }
        if let Some(uuid) = archived.iter().find(|uuid| self.uuid_index.contains_key(uuid)).copied() {
            return Err(ProjectPersistenceError::Codec {
                node_type: projects
                    .first()
                    .map(|project| project.root.node_type.clone())
                    .unwrap_or_default(),
                message: format!(
                    "cannot restore archived subtrees because UUID {} is already live",
                    uuid.0
                ),
            });
        }
        let decoded = {
            let parent_node = self
                .nodes
                .get(parent)
                .ok_or(ProjectPersistenceError::MissingNode(parent))?;
            projects
                .iter()
                .map(|project| {
                    Self::decode_project_record_tree_with(parent_node, &project.root, &HashMap::new(), &mut decode_node)
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        let checkpoint = self.edit_checkpoint();
        let mut roots = Vec::with_capacity(decoded.len());
        let mut previous = prev_sibling;
        for tree in decoded {
            match self.insert_decoded_project_tree(parent, previous, tree, "RestoreProjectSubtrees") {
                Ok(root) => {
                    roots.push(root);
                    previous = Some(root);
                }
                Err(error) => {
                    return Err(self.abort_project_subtree_commit(checkpoint, error));
                }
            }
        }
        if let Err(error) =
            self.replay_loaded_subtrees_lifecycle(&roots, NodeCreationContext::ProjectLoad, LoadedReadyMode::Immediate)
        {
            return Err(self.abort_project_subtree_commit(checkpoint, error));
        }

        let mut loaded_by_root = Vec::with_capacity(roots.len());
        let mut all_loaded = Vec::new();
        for root in &roots {
            match self.collect_loaded_subtree_node_ids(*root) {
                Ok(nodes) => {
                    all_loaded.extend(nodes.iter().copied());
                    loaded_by_root.push(nodes);
                }
                Err(error) => {
                    return Err(self.abort_project_subtree_commit(checkpoint, error));
                }
            }
        }
        self.sync_missing_reference_warnings_for_nodes_silent(all_loaded.as_slice());
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();

        let catalog_snapshot = all_loaded
            .iter()
            .any(|node| self.catalog_creatable_items_require_tree_snapshot(*node))
            .then(|| self.build_process_tree_snapshot());
        let groups = loaded_by_root.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let ops = match self.loaded_forest_ui_ops(&groups, catalog_snapshot.as_deref()) {
            Ok(ops) => ops,
            Err(error) => return Err(self.abort_project_subtree_commit(checkpoint, error)),
        };
        self.push_ui_graph_transaction(ops);
        for root in &roots {
            self.record_single_history_step(
                AddNodeEffect {
                    node: *root,
                    parent,
                    prev_sibling: self.nodes.get(*root).and_then(|node| node.node_data().prev_sibling),
                    next_sibling: self.nodes.get(*root).and_then(|node| node.node_data().next_sibling),
                }
                .into(),
            );
        }
        self.commit_edit_checkpoint(checkpoint);
        Ok(roots)
    }

    fn push_loaded_subtree_ui_events(&mut self, node_ids: &[NodeId]) -> Result<(), ProjectPersistenceError> {
        let catalog_snapshot = node_ids
            .iter()
            .any(|node_id| self.catalog_creatable_items_require_tree_snapshot(*node_id))
            .then(|| self.build_process_tree_snapshot());
        let ops = self.loaded_forest_ui_ops(&[node_ids], catalog_snapshot.as_deref())?;
        self.push_ui_graph_transaction(ops);
        Ok(())
    }

    fn loaded_forest_ui_ops(
        &self,
        node_groups: &[&[NodeId]],
        catalog_snapshot: Option<&ProcessTreeSnapshot>,
    ) -> Result<Vec<UiGraphOp>, ProjectPersistenceError> {
        let mut ops = Vec::<UiGraphOp>::new();
        let mut parents = Vec::new();
        let mut order_ops = HashMap::new();
        const SUBTREE_COMPACT_THRESHOLD: usize = 8;
        for node_ids in node_groups {
            let Some(root) = node_ids.first().copied() else {
                continue;
            };
            let parent = self
                .nodes
                .get(root)
                .ok_or(ProjectPersistenceError::MissingNode(root))?
                .node_data()
                .parent;
            let compact = parent.is_some() && (node_groups.len() > 1 || node_ids.len() > SUBTREE_COMPACT_THRESHOLD);
            let mut snapshots = Vec::new();
            for node in *node_ids {
                let snapshot = match catalog_snapshot {
                    Some(catalog_snapshot) => self.ui_node_dto_for_event_with_catalog_snapshot(*node, catalog_snapshot),
                    None => self.ui_node_dto_for_event_without_catalog_snapshot(*node),
                }
                .ok_or(ProjectPersistenceError::MissingNode(*node))?;
                if compact {
                    snapshots.push(snapshot);
                } else {
                    let parent = self.nodes.get(*node).unwrap().node_data().parent;
                    let index = parent.and_then(|parent| self.ui_child_index(parent, *node));
                    ops.push(UiGraphOp::NodeCreated {
                        snapshot: Box::new(snapshot),
                        parent,
                        index,
                    });
                }
            }
            if let Some(parent) = parent {
                if !order_ops.contains_key(&parent) {
                    parents.push(parent);
                }
                order_ops.insert(parent, compact.then_some(ops.len()));
            }
            if compact {
                ops.push(UiGraphOp::SubtreeInserted {
                    root,
                    parent: parent.unwrap(),
                    nodes: snapshots,
                    parent_children_after: None,
                });
            }
        }
        // Build each sibling order once, before the transaction compacts it to a splice.
        // Encoding and then discarding one full sibling list per root is still quadratic.
        for parent in parents {
            let children = self.ui_direct_children(parent).unwrap_or_default();
            if let Some(index) = order_ops[&parent] {
                let UiGraphOp::SubtreeInserted {
                    parent_children_after, ..
                } = &mut ops[index]
                else {
                    unreachable!()
                };
                *parent_children_after = Some(children);
            } else {
                ops.push(UiGraphOp::ChildrenReordered { parent, children });
            }
        }
        Ok(ops)
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
        let parent_node = self
            .nodes
            .get(parent)
            .ok_or(ProjectPersistenceError::MissingNode(parent))?;
        let migrated =
            parent_node
                .project_migrate_children(children)
                .map_err(|message| ProjectPersistenceError::Codec {
                    node_type: parent_node.get_type().to_owned(),
                    message,
                })?;
        let mut prev_sibling = None;

        for child_record in migrated.iter() {
            let mut child = {
                let parent_node = self
                    .nodes
                    .get(parent)
                    .ok_or(ProjectPersistenceError::MissingNode(parent))?;
                Self::decode_node_record_with(Some(parent_node), child_record, decode_node)?
            };
            let parent_enabled = self
                .nodes
                .get(parent)
                .ok_or(ProjectPersistenceError::MissingNode(parent))?
                .node_data()
                .effective_enabled;
            let effective_enabled = parent_enabled && child.node_data().meta.enabled;
            child.node_data_mut().effective_enabled = effective_enabled;
            let child_id = self.nodes.insert(child);
            self.register_node_uuid(child_id);
            self.attach_node(0, "LoadProject", child_id, parent, prev_sibling)?;
            self.populate_param_cache_entry(child_id);
            self.load_children_records(child_id, &child_record.children, decode_node)?;
            prev_sibling = Some(child_id);
        }

        Ok(())
    }

    fn validate_persisted_subtree_destination(
        &self,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        operation: &'static str,
    ) -> Result<(), ProjectPersistenceError> {
        if !self.nodes.contains(parent) {
            return Err(ProjectPersistenceError::MissingNode(parent));
        }

        let Some(prev_sibling) = prev_sibling else {
            return Ok(());
        };
        let sibling_parent = self
            .nodes
            .get(prev_sibling)
            .ok_or(EngineEditError::SiblingNotFound {
                edit_index: 0,
                operation,
                sibling: prev_sibling,
            })?
            .node_data()
            .parent;
        if sibling_parent != Some(parent) {
            return Err(EngineEditError::InvalidSiblingParent {
                edit_index: 0,
                operation,
                parent,
                sibling: prev_sibling,
                sibling_parent,
            }
            .into());
        }

        Ok(())
    }

    fn decode_project_record_tree_with<F>(
        parent: &T,
        record: &ProjectNodeRecord,
        uuid_map: &HashMap<NodeUuid, NodeUuid>,
        decode_node: &mut F,
    ) -> Result<DecodedProjectTree<T>, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut node = Self::decode_node_record_with(Some(parent), record, decode_node)?;
        remap_node_references(&mut node, uuid_map);

        let migrated =
            node.project_migrate_children(&record.children)
                .map_err(|message| ProjectPersistenceError::Codec {
                    node_type: record.node_type.clone(),
                    message,
                })?;
        let mut children = Vec::with_capacity(migrated.len());
        for child_record in migrated.iter() {
            children.push(Self::decode_project_record_tree_with(
                &node,
                child_record,
                uuid_map,
                decode_node,
            )?);
        }

        Ok(DecodedProjectTree { node, children })
    }

    fn insert_decoded_project_tree(
        &mut self,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        tree: DecodedProjectTree<T>,
        operation: &'static str,
    ) -> Result<NodeId, ProjectPersistenceError> {
        let journal_order = self.edit_order();
        self.validate_previous_sibling(0, operation, None, parent, prev_sibling)?;
        let mut node = tree.node;
        let parent_enabled = self
            .nodes
            .get(parent)
            .ok_or(ProjectPersistenceError::MissingNode(parent))?
            .node_data()
            .effective_enabled;
        let effective_enabled = parent_enabled && node.node_data().meta.enabled;
        node.node_data_mut().effective_enabled = effective_enabled;
        let node_id = self.nodes.insert(node);
        self.register_node_uuid(node_id);
        self.attach_node(0, operation, node_id, parent, prev_sibling)?;
        self.populate_param_cache_entry(node_id);
        if let Some(order) = journal_order {
            let data = self.nodes.get(node_id).expect("inserted node exists").node_data();
            let effect = AddNodeEffect {
                node: node_id,
                parent,
                prev_sibling: data.prev_sibling,
                next_sibling: data.next_sibling,
            };
            self.capture_journal_step(order, effect.into());
        }

        let mut child_prev_sibling = None;
        for child in tree.children {
            let child_id = self.insert_decoded_project_tree(node_id, child_prev_sibling, child, operation)?;
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

fn collect_record_uuids(
    record: &ProjectNodeRecord,
    uuids: &mut HashSet<NodeUuid>,
) -> Result<(), ProjectPersistenceError> {
    if !uuids.insert(record.uuid) {
        return Err(ProjectPersistenceError::Codec {
            node_type: record.node_type.clone(),
            message: format!("archived subtree contains duplicate UUID {}", record.uuid.0),
        });
    }
    for child in &record.children {
        collect_record_uuids(child, uuids)?;
    }
    Ok(())
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
