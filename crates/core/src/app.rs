//! App lifecycle hooks and host-independent runtime preparation.

use std::collections::HashSet;
use std::fs;
use std::io::Error;
use std::path::Path;
use std::sync::Arc;

use crate::engine::{
    Engine, EngineRuntimeError, PROJECT_FILE_VERSION, ProjectFile, ProjectNodeMeta, ProjectNodeRecord,
    ProjectPersistenceError,
};
use crate::node::{DashboardNode, Folder, Node, NodeId, NodeMeta, NodeUuid, UserNodeRole};
use crate::parameter::Parameter;
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

/// App-provided project file metadata consumed by hosts and UIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFileSpec {
    /// Human-readable name for one project document, such as `Noisette`.
    pub display_name: &'static str,
    /// Preferred filename extension without a leading dot.
    pub extension: &'static str,
}

impl ProjectFileSpec {
    /// Creates one project-file descriptor.
    pub const fn new(display_name: &'static str, extension: &'static str) -> Self {
        Self {
            display_name,
            extension,
        }
    }

    /// Returns the normalized extension used by hosts and transports.
    pub fn normalized_extension(&self) -> String {
        let normalized = self.extension.trim().trim_start_matches('.').to_ascii_lowercase();
        if normalized.is_empty() {
            return "json".to_string();
        }
        normalized
    }

    /// Returns the human-readable label, falling back to a generic default.
    pub fn normalized_display_name(&self) -> String {
        let normalized = self.display_name.trim();
        if normalized.is_empty() {
            return "Project".to_string();
        }
        normalized.to_string()
    }
}

impl Default for ProjectFileSpec {
    fn default() -> Self {
        Self::new("Project", "json")
    }
}

/// App node contract required by the default runtime host.
pub trait ProjectNode: Node {
    /// Creates one node from its default constructor path without applying persisted data.
    fn project_create_node(node_type: &str) -> Option<Self>
    where
        Self: Sized;

    /// Recreates one node from project persistence payload.
    fn project_decode_node(node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<Self, String>
    where
        Self: Sized;
}

/// App-controlled lifecycle hooks for engine recreation and new-project setup.
pub trait ProjectLifecycle: ProjectNode + From<Folder> + From<DashboardNode> {
    /// Creates the root node used for fresh engine instances.
    fn create_project_root() -> Self {
        Folder::new("Root").into()
    }

    /// Declares the project file label and extension used by hosts and UIs.
    fn project_file_spec() -> ProjectFileSpec {
        ProjectFileSpec::default()
    }

    /// Applies runtime-only engine setup that must run on every recreation.
    fn configure_engine(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }

    /// Seeds a fresh project with default content.
    fn initialize_new_project(engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        add_default_project_nodes(engine);
        Ok(())
    }

    /// Applies post-load setup after deserializing a project file.
    fn project_opened(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }
}

/// Queues the core default nodes that every fresh project should start with.
pub fn add_default_project_nodes<T>(engine: &mut Engine<T>)
where
    T: Node + From<DashboardNode>,
{
    engine.add_node(DashboardNode::new().into(), None);
}

/// Creates a fresh engine instance and applies the app runtime configuration.
pub fn create_engine<T>() -> Result<Engine<T>, String>
where
    T: ProjectLifecycle,
{
    let mut engine = Engine::new(T::create_project_root());
    T::configure_engine(&mut engine)?;
    Ok(engine)
}

/// Creates a fresh engine instance and seeds a new project template.
pub fn create_new_project_engine<T>() -> Result<Engine<T>, String>
where
    T: ProjectLifecycle,
{
    let mut engine = create_engine::<T>()?;
    T::initialize_new_project(&mut engine)?;
    Ok(engine)
}

/// Reapplies app-owned runtime configuration after deserializing a project.
pub fn configure_loaded_engine<T>(engine: &mut Engine<T>) -> Result<(), String>
where
    T: ProjectLifecycle,
{
    T::configure_engine(engine)?;
    T::project_opened(engine)
}

/// Applies startup edits, resolves scheduling, and clears bootstrap-only runtime state.
pub fn prepare_engine_for_runtime<T: Node>(engine: &mut Engine<T>) -> std::io::Result<()> {
    engine
        .apply_edits()
        .map_err(|err| Error::other(format!("initial apply_edits failed: {err}")))?;
    engine
        .run_pending_node_ready_callbacks()
        .map_err(|err| Error::other(format!("initial node-ready callbacks failed: {err}")))?;
    engine
        .resolve_if_needed()
        .map_err(|err| Error::other(format!("initial resolve failed: {err}")))?;
    // Startup shape is already reflected by the in-memory graph and initial snapshot.
    // Dropping bootstrap inbox events avoids a very expensive first runtime tick for large graphs.
    engine.inbox.clear();
    engine.clear_history(); // keep runtime undo history strictly post-start
    Ok(())
}

/// Runs node destroy callbacks before discarding or replacing one live engine.
///
/// Any edits queued during destroy are intentionally ignored because the engine is about to be dropped.
pub fn shutdown_engine_for_runtime<T: Node>(engine: &mut Engine<T>) {
    let Ok(mut node_ids) = engine.collect_subtree(0, "ShutdownEngine", engine.root) else {
        return;
    };
    node_ids.reverse();

    let tree_snapshot = engine.build_process_tree_snapshot();

    for node_id in node_ids {
        let mut destroy_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        destroy_ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

        if let Some(node) = engine.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.destroy(&mut destroy_ctx);
            });
        }
    }
}

/// Serializes one project using the app node codec path used for project files.
pub fn to_sparse_project_json_pretty<T>(engine: &Engine<T>) -> Result<String, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let project = to_sparse_project_file(engine)?;
    Ok(serde_json::to_string_pretty(&project)?)
}

/// Serializes one node subtree using the same sparse codec as project files.
pub fn to_sparse_subtree_json_pretty<T>(engine: &Engine<T>, root: NodeId) -> Result<String, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let project = to_sparse_subtree_file(engine, root)?;
    Ok(serde_json::to_string_pretty(&project)?)
}

/// Writes one sparse project file that omits default-backed child records.
pub fn save_sparse_project_file<T, P>(engine: &Engine<T>, path: P) -> Result<(), ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
    P: AsRef<Path>,
{
    let json = to_sparse_project_json_pretty(engine)?;
    fs::write(path, json)?;
    Ok(())
}

/// Loads one sparse project JSON document by first expanding declared deltas
/// against the app-owned schema defaults.
pub fn from_sparse_project_json<T>(json: &str) -> Result<Engine<T>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let project: ProjectFile = serde_json::from_str(json)?;
    if project.version != PROJECT_FILE_VERSION {
        return Err(ProjectPersistenceError::UnsupportedVersion {
            found: project.version,
            expected: PROJECT_FILE_VERSION,
        });
    }
    let expanded = expand_sparse_project_file::<T>(project)?;
    Engine::<T>::from_project_file_with(expanded, T::project_decode_node)
}

/// Loads one sparse project file by first expanding declared deltas against the
/// app-owned schema defaults.
pub fn load_sparse_project_file<T, P>(path: P) -> Result<Engine<T>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
    P: AsRef<Path>,
{
    let json = fs::read_to_string(path)?;
    from_sparse_project_json(&json)
}

/// Imports one sparse persisted subtree beneath `parent`.
pub fn insert_sparse_subtree_json<T>(
    engine: &mut Engine<T>,
    parent: NodeId,
    prev_sibling: Option<NodeId>,
    json: &str,
) -> Result<NodeId, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let project: ProjectFile = serde_json::from_str(json)?;
    if project.version != PROJECT_FILE_VERSION {
        return Err(ProjectPersistenceError::UnsupportedVersion {
            found: project.version,
            expected: PROJECT_FILE_VERSION,
        });
    }
    let expanded = expand_sparse_project_file::<T>(project)?;
    engine.insert_project_subtree_with(expanded, parent, prev_sibling, T::project_decode_node)
}

fn to_sparse_project_file<T>(engine: &Engine<T>) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let referenced_uuids = collect_referenced_uuids(engine);
    let structural_root = build_structural_baseline_record_for_node(engine, engine.root)?;
    let root = encode_sparse_node_record(engine, engine.root, Some(&structural_root), false, &referenced_uuids)?
        .ok_or_else(|| ProjectPersistenceError::Codec {
            node_type: engine
                .nodes
                .get(engine.root)
                .map(Node::get_type)
                .unwrap_or("unknown")
                .to_string(),
            message: "root node cannot be omitted from sparse project output".to_string(),
        })?;

    Ok(ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        root,
    })
}

fn to_sparse_subtree_file<T>(engine: &Engine<T>, root_id: NodeId) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let referenced_uuids = collect_referenced_uuids(engine);
    let structural_root = build_structural_baseline_record_for_node(engine, root_id)?;
    let root = encode_sparse_node_record(engine, root_id, Some(&structural_root), false, &referenced_uuids)?
        .ok_or_else(|| ProjectPersistenceError::Codec {
            node_type: engine
                .nodes
                .get(root_id)
                .map(Node::get_type)
                .unwrap_or("unknown")
                .to_string(),
            message: "subtree root cannot be omitted from sparse output".to_string(),
        })?;

    Ok(ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        root,
    })
}

fn expand_sparse_project_file<T>(project: ProjectFile) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    Ok(ProjectFile {
        version: project.version,
        root: expand_sparse_node_record::<T>(&project.root, None, None, None)?,
    })
}

fn expand_sparse_node_record<T>(
    record: &ProjectNodeRecord,
    parent_node: Option<&T>,
    parent_record: Option<&ProjectNodeRecord>,
    matched_baseline: Option<&ProjectNodeRecord>,
) -> Result<ProjectNodeRecord, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let baseline_storage;
    let (mut expanded, baseline_children) = match matched_baseline {
        Some(baseline) => (
            merge_sparse_record_with_baseline(baseline, record),
            baseline.children.as_slice(),
        ),
        None => {
            baseline_storage =
                build_structural_baseline_record_from_expanded_record::<T>(record, parent_node, parent_record)?;
            (
                merge_sparse_record_with_baseline(&baseline_storage, record),
                baseline_storage.children.as_slice(),
            )
        }
    };
    let current_node = decode_sparse_baseline_node(parent_node, &expanded)?;

    expanded.children = expand_sparse_child_records(record, &current_node, &expanded, baseline_children)?;

    Ok(expanded)
}

fn build_structural_baseline_record_from_expanded_record<T>(
    record: &ProjectNodeRecord,
    parent_node: Option<&T>,
    parent_record: Option<&ProjectNodeRecord>,
) -> Result<ProjectNodeRecord, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let recreated = decode_sparse_baseline_node(parent_node, record)?;
    let attach_under_parent = record.user_role == UserNodeRole::ItemRoot
        && parent_node.is_some_and(|parent| {
            parent.user_container_accepts_item(record.node_type.as_str(), recreated.user_item_kind())
        });
    let mut temp = Engine::new(Folder::new("Sparse Expand Root").into());

    if attach_under_parent {
        let Some(parent_record) = parent_record else {
            return Err(ProjectPersistenceError::Codec {
                node_type: record.node_type.clone(),
                message: "sparse expansion cannot materialize an item baseline without its parent record".to_string(),
            });
        };
        let parent = decode_sparse_baseline_node::<T>(None, parent_record)?;
        temp.add_node(parent, None);
        temp.apply_edits_without_creation_callbacks()?;
        let temp_parent = temp
            .nodes
            .get(temp.root)
            .and_then(|root| root.node_data().first_child)
            .ok_or_else(|| ProjectPersistenceError::Codec {
                node_type: record.node_type.clone(),
                message: "sparse expansion parent baseline did not materialize".to_string(),
            })?;
        temp.add_user_item(recreated, Some(temp_parent));
    } else {
        temp.add_node(recreated, None);
    }

    temp.apply_edits_without_creation_callbacks()?;

    let baseline = temp.to_project_file_with(|node| node.project_encode_data())?;
    if attach_under_parent {
        return baseline
            .root
            .children
            .into_iter()
            .next()
            .and_then(|parent| {
                let mut children = parent.children.into_iter();
                let first = children.next()?;
                if first.uuid == record.uuid {
                    Some(first)
                } else {
                    children.find(|child| child.uuid == record.uuid).or(Some(first))
                }
            })
            .ok_or_else(|| ProjectPersistenceError::Codec {
                node_type: record.node_type.clone(),
                message: "sparse expansion item baseline did not materialize the target record".to_string(),
            });
    }

    baseline
        .root
        .children
        .into_iter()
        .next()
        .ok_or_else(|| ProjectPersistenceError::Codec {
            node_type: record.node_type.clone(),
            message: "sparse expansion baseline did not materialize the target node record".to_string(),
        })
}

fn decode_sparse_baseline_node<T>(
    parent_node: Option<&T>,
    record: &ProjectNodeRecord,
) -> Result<T, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let data = record.data.clone().unwrap_or(serde_json::Value::Null);
    let meta = record.meta.clone().into_runtime(record.uuid);

    let mut node = if record.user_role == UserNodeRole::ItemRoot {
        if let Some(parent) = parent_node {
            if let Some(mut node) = parent.create_user_item(record.node_type.as_str()) {
                node.project_decode_data(&data)
                    .map_err(|message| ProjectPersistenceError::Codec {
                        node_type: record.node_type.clone(),
                        message,
                    })?;
                T::from_boxed_node(node).ok_or_else(|| ProjectPersistenceError::Codec {
                    node_type: record.node_type.clone(),
                    message: "parent item factory returned a node outside the engine node enum".to_string(),
                })?
            } else {
                decode_sparse_node_without_parent_factory(record, &data, &meta)?
            }
        } else {
            decode_sparse_node_without_parent_factory(record, &data, &meta)?
        }
    } else {
        decode_sparse_node_without_parent_factory(record, &data, &meta)?
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

fn decode_sparse_node_without_parent_factory<T>(
    record: &ProjectNodeRecord,
    data: &serde_json::Value,
    meta: &NodeMeta,
) -> Result<T, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    if let Some(mut node) = T::project_create_node(record.node_type.as_str()) {
        node.project_decode_data(data)
            .map_err(|message| ProjectPersistenceError::Codec {
                node_type: record.node_type.clone(),
                message,
            })?;
        return Ok(node);
    }

    T::project_decode_node(record.node_type.as_str(), data, meta).map_err(|message| ProjectPersistenceError::Codec {
        node_type: record.node_type.clone(),
        message,
    })
}

fn merge_sparse_record_with_baseline(baseline: &ProjectNodeRecord, overlay: &ProjectNodeRecord) -> ProjectNodeRecord {
    ProjectNodeRecord {
        uuid: overlay.uuid,
        node_type: overlay.node_type.clone(),
        user_role: overlay.user_role,
        meta: baseline.meta.merged_with_sparse_overlay(&overlay.meta),
        data: merge_sparse_project_data(baseline.data.as_ref(), overlay.data.as_ref()),
        children: overlay.children.clone(),
    }
}

fn merge_sparse_project_data(
    baseline: Option<&serde_json::Value>,
    overlay: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    match (baseline, overlay) {
        (Some(baseline), Some(overlay)) => Some(merge_sparse_json_value(baseline, overlay)),
        (Some(baseline), None) => Some(baseline.clone()),
        (None, Some(overlay)) => Some(overlay.clone()),
        (None, None) => None,
    }
}

fn merge_sparse_json_value(baseline: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (baseline.as_object(), overlay.as_object()) {
        (Some(baseline), Some(overlay)) => {
            let mut merged = baseline.clone();
            for (key, value) in overlay {
                merged.insert(key.clone(), value.clone());
            }
            serde_json::Value::Object(merged)
        }
        _ => overlay.clone(),
    }
}

fn expand_sparse_child_records<T>(
    record: &ProjectNodeRecord,
    current_node: &T,
    expanded_parent_record: &ProjectNodeRecord,
    baseline_children: &[ProjectNodeRecord],
) -> Result<Vec<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let mut consumed_overlay_children = vec![false; record.children.len()];
    let mut expanded_children = Vec::with_capacity(baseline_children.len().max(record.children.len()));

    // Saved sparse children are overlays. Declared baseline children own the structural order.
    for baseline_child in baseline_children {
        let matched_index = record
            .children
            .iter()
            .enumerate()
            .find(|(index, child)| {
                !consumed_overlay_children[*index] && sparse_child_matches_baseline(child, baseline_child)
            })
            .map(|(index, _)| index);

        if let Some(index) = matched_index {
            consumed_overlay_children[index] = true;
            expanded_children.push(expand_sparse_node_record(
                &record.children[index],
                Some(current_node),
                Some(expanded_parent_record),
                Some(baseline_child),
            )?);
        } else {
            expanded_children.push(baseline_child.clone());
        }
    }

    for (index, child) in record.children.iter().enumerate() {
        if consumed_overlay_children[index] {
            continue;
        }

        expanded_children.push(expand_sparse_node_record(
            child,
            Some(current_node),
            Some(expanded_parent_record),
            None,
        )?);
    }

    Ok(expanded_children)
}

fn sparse_child_matches_baseline(child_record: &ProjectNodeRecord, baseline_child: &ProjectNodeRecord) -> bool {
    let Some(child_decl_id) = child_record.meta.decl_id.as_ref() else {
        return false;
    };

    baseline_child.node_type == child_record.node_type
        && baseline_child.user_role == child_record.user_role
        && baseline_child.meta.decl_id.as_ref() == Some(child_decl_id)
}

fn encode_sparse_node_record<T>(
    engine: &Engine<T>,
    node_id: NodeId,
    matched_parent_baseline: Option<&ProjectNodeRecord>,
    allow_omission: bool,
    referenced_uuids: &HashSet<NodeUuid>,
) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let node = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;

    let node_type = node.get_type().to_string();
    let user_role = node.node_data().user_role;
    let matched_parent_baseline =
        matched_parent_baseline.filter(|baseline| baseline.node_type == node_type && baseline.user_role == user_role);

    let self_matches_parent_baseline = if let Some(baseline) = matched_parent_baseline {
        project_meta_delta_from_runtime(&node.node_data().meta, Some(&baseline.meta)).is_empty()
            && project_persisted_data_from_runtime(node, Some(baseline))?.is_none()
    } else {
        false
    };

    let structural_baseline_storage;
    let baseline_children = if self_matches_parent_baseline {
        matched_parent_baseline
            .map(|baseline| baseline.children.as_slice())
            .unwrap_or(&[])
    } else {
        structural_baseline_storage = Some(build_structural_baseline_record_for_node(engine, node_id)?);
        structural_baseline_storage
            .as_ref()
            .map(|baseline| baseline.children.as_slice())
            .unwrap_or(&[])
    };

    let default_baseline_storage;
    let self_baseline = if let Some(baseline) = matched_parent_baseline {
        Some(baseline)
    } else {
        default_baseline_storage = build_default_baseline_record_for_node(engine, node_id)?;
        default_baseline_storage.as_ref()
    };

    let mut child_ids = Vec::new();
    let mut child = node.node_data().first_child;
    while let Some(child_id) = child {
        child_ids.push(child_id);
        child = engine
            .nodes
            .get(child_id)
            .ok_or(ProjectPersistenceError::MissingNode(child_id))?
            .node_data()
            .next_sibling;
    }

    let mut children = Vec::new();
    for child_id in child_ids {
        let child_node = engine
            .nodes
            .get(child_id)
            .ok_or(ProjectPersistenceError::MissingNode(child_id))?;
        let matched_child_baseline = find_matching_child_baseline(child_node, baseline_children);
        if let Some(record) = encode_sparse_node_record(
            engine,
            child_id,
            matched_child_baseline,
            matched_child_baseline.is_some(),
            referenced_uuids,
        )? {
            children.push(record);
        }
    }

    if allow_omission
        && self_matches_parent_baseline
        && children.is_empty()
        && !referenced_uuids.contains(&node.node_data().meta.uuid)
    {
        return Ok(None);
    }

    let meta = project_meta_for_sparse_record(
        &node.node_data().meta,
        matched_parent_baseline.map(|baseline| &baseline.meta),
        self_baseline.map(|baseline| &baseline.meta),
        allow_omission && matched_parent_baseline.is_some(),
    );
    let data = project_data_for_sparse_record(
        node,
        matched_parent_baseline,
        self_baseline,
        allow_omission && matched_parent_baseline.is_some(),
    )?;

    Ok(Some(ProjectNodeRecord {
        uuid: node.node_data().meta.uuid,
        node_type,
        user_role,
        meta,
        data,
        children,
    }))
}

fn build_structural_baseline_record_for_node<T>(
    engine: &Engine<T>,
    node_id: NodeId,
) -> Result<ProjectNodeRecord, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current_node = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
    let data = current_node
        .project_encode_data()
        .map_err(|message| ProjectPersistenceError::Codec {
            node_type: current_node.get_type().to_string(),
            message,
        })?;
    let recreated = recreate_node_from_current_record(engine, node_id, &data)?;

    materialize_baseline_record_for_recreated_node(engine, node_id, recreated, "Sparse Baseline Root")
}

fn build_default_baseline_record_for_node<T>(
    engine: &Engine<T>,
    node_id: NodeId,
) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let Some(recreated) = recreate_default_node_from_current_record(engine, node_id)? else {
        return Ok(None);
    };

    materialize_baseline_record_for_recreated_node(engine, node_id, recreated, "Sparse Default Baseline Root").map(Some)
}

fn materialize_baseline_record_for_recreated_node<T>(
    engine: &Engine<T>,
    node_id: NodeId,
    recreated: T,
    root_label: &str,
) -> Result<ProjectNodeRecord, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
    let current_uuid = current.node_data().meta.uuid;
    let node_type = current.get_type().to_string();
    let parent_id = current.node_data().parent;
    let attach_under_parent = current.node_data().user_role == UserNodeRole::ItemRoot
        && parent_id
            .and_then(|parent_id| engine.nodes.get(parent_id))
            .is_some_and(|parent| parent.user_container_accepts_item(current.get_type(), current.user_item_kind()));

    let mut temp = Engine::new(Folder::new(root_label).into());
    if attach_under_parent {
        let parent_id = parent_id.expect("item roots with attach_under_parent should have a parent");
        let parent = recreate_parent_for_baseline(engine, parent_id)?;
        temp.add_node(parent, None);
        temp.apply_edits_without_creation_callbacks()?;
        let temp_parent = temp
            .nodes
            .get(temp.root)
            .and_then(|root| root.node_data().first_child)
            .ok_or_else(|| ProjectPersistenceError::Codec {
                node_type: node_type.clone(),
                message: "baseline parent did not materialize".to_string(),
            })?;
        temp.add_user_item(recreated.into(), Some(temp_parent));
    } else {
        temp.add_node(recreated, None);
    }

    temp.apply_edits_without_creation_callbacks()?;

    let baseline = temp.to_project_file_with(|node| node.project_encode_data())?;
    if attach_under_parent {
        return baseline
            .root
            .children
            .into_iter()
            .next()
            .and_then(|parent| {
                let mut children = parent.children.into_iter();
                let first = children.next()?;
                if first.uuid == current_uuid {
                    Some(first)
                } else {
                    children.find(|child| child.uuid == current_uuid).or(Some(first))
                }
            })
            .ok_or_else(|| ProjectPersistenceError::Codec {
                node_type,
                message: "nested baseline did not materialize the target node record".to_string(),
            });
    }

    baseline
        .root
        .children
        .into_iter()
        .next()
        .ok_or_else(|| ProjectPersistenceError::Codec {
            node_type,
            message: "baseline did not materialize a node record".to_string(),
        })
}

fn recreate_parent_for_baseline<T>(engine: &Engine<T>, parent_id: NodeId) -> Result<T, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let parent = engine
        .nodes
        .get(parent_id)
        .ok_or(ProjectPersistenceError::MissingNode(parent_id))?;
    let node_type = parent.get_type().to_string();
    let data = parent
        .project_encode_data()
        .map_err(|message| ProjectPersistenceError::Codec {
            node_type: node_type.clone(),
            message,
        })?;

    recreate_node_from_current_record(engine, parent_id, &data)
}

fn recreate_node_from_current_record<T>(
    engine: &Engine<T>,
    node_id: NodeId,
    data: &serde_json::Value,
) -> Result<T, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
    let node_type = current.get_type().to_string();
    let current_meta = current.node_data().meta.clone();

    let mut recreated = if current.node_data().user_role == UserNodeRole::ItemRoot {
        if let Some(parent_id) = current.node_data().parent {
            let parent = engine
                .nodes
                .get(parent_id)
                .ok_or(ProjectPersistenceError::MissingNode(parent_id))?;
            if let Some(mut node) = parent.create_user_item(node_type.as_str()) {
                node.node_data_mut().meta.label = current_meta.label.clone();
                node.project_decode_data(data)
                    .map_err(|message| ProjectPersistenceError::Codec {
                        node_type: node_type.clone(),
                        message,
                    })?;
                T::from_boxed_node(node).ok_or_else(|| ProjectPersistenceError::Codec {
                    node_type: node_type.clone(),
                    message: "parent item factory returned a node outside the engine node enum".to_string(),
                })?
            } else {
                T::project_decode_node(node_type.as_str(), data, &current_meta).map_err(|message| {
                    ProjectPersistenceError::Codec {
                        node_type: node_type.clone(),
                        message,
                    }
                })?
            }
        } else {
            T::project_decode_node(node_type.as_str(), data, &current_meta).map_err(|message| {
                ProjectPersistenceError::Codec {
                    node_type: node_type.clone(),
                    message,
                }
            })?
        }
    } else {
        T::project_decode_node(node_type.as_str(), data, &current_meta).map_err(|message| {
            ProjectPersistenceError::Codec {
                node_type: node_type.clone(),
                message,
            }
        })?
    };

    let recreated_data = recreated.node_data_mut();
    recreated_data.parent = None;
    recreated_data.first_child = None;
    recreated_data.last_child = None;
    recreated_data.prev_sibling = None;
    recreated_data.next_sibling = None;
    recreated_data.user_role = current.node_data().user_role;
    recreated_data.meta = current_meta;

    Ok(recreated)
}

fn recreate_default_node_from_current_record<T>(
    engine: &Engine<T>,
    node_id: NodeId,
) -> Result<Option<T>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
    let node_type = current.get_type().to_string();

    if current.node_data().user_role == UserNodeRole::ItemRoot {
        if let Some(parent_id) = current.node_data().parent {
            let parent = engine
                .nodes
                .get(parent_id)
                .ok_or(ProjectPersistenceError::MissingNode(parent_id))?;
            if let Some(node) = parent.create_user_item(node_type.as_str()) {
                return T::from_boxed_node(node)
                    .ok_or_else(|| ProjectPersistenceError::Codec {
                        node_type,
                        message: "parent item factory returned a node outside the engine node enum".to_string(),
                    })
                    .map(Some);
            }
        }
    }

    Ok(T::project_create_node(node_type.as_str()))
}

fn find_matching_child_baseline<'a, T: Node>(
    child_node: &T,
    baseline_children: &'a [ProjectNodeRecord],
) -> Option<&'a ProjectNodeRecord> {
    baseline_children.iter().find(|baseline_child| {
        baseline_child.node_type == child_node.get_type()
            && baseline_child.user_role == child_node.node_data().user_role
            && baseline_child.meta.decl_id.as_ref() == Some(&child_node.node_data().meta.decl_id)
    })
}

fn project_meta_from_runtime(meta: &NodeMeta) -> ProjectNodeMeta {
    ProjectNodeMeta::from_runtime(meta).without_runtime_fields()
}

fn project_meta_for_sparse_record(
    meta: &NodeMeta,
    matched_parent_baseline: Option<&ProjectNodeMeta>,
    self_baseline: Option<&ProjectNodeMeta>,
    use_declared_overlay_delta: bool,
) -> ProjectNodeMeta {
    if use_declared_overlay_delta {
        let mut persisted = project_meta_delta_from_runtime(meta, matched_parent_baseline);
        persisted.decl_id = Some(meta.decl_id.clone());
        return persisted;
    }

    if matched_parent_baseline.is_some() {
        let mut persisted = project_meta_from_runtime(meta);
        persisted.description = None;
        persisted.declared_description_key = None;
        persisted.declared_description = None;
        if persisted.tags.as_ref().is_some_and(Vec::is_empty) {
            persisted.tags = None;
        }
        if persisted
            .user_permissions
            .as_ref()
            .is_some_and(|value| *value == Default::default())
        {
            persisted.user_permissions = None;
        }
        if persisted
            .semantics
            .as_ref()
            .is_some_and(|value| *value == Default::default())
        {
            persisted.semantics = None;
        }
        if persisted
            .presentation
            .as_ref()
            .is_some_and(|value| *value == Default::default())
        {
            persisted.presentation = None;
        }
        return persisted;
    }

    project_meta_delta_from_runtime(meta, self_baseline)
}

fn project_meta_delta_from_runtime(meta: &NodeMeta, baseline: Option<&ProjectNodeMeta>) -> ProjectNodeMeta {
    let current = project_meta_from_runtime(meta);
    let Some(baseline) = baseline else {
        return current;
    };

    current.delta_against(&baseline.without_runtime_fields())
}

fn project_data_for_sparse_record<T: Node>(
    node: &T,
    matched_parent_baseline: Option<&ProjectNodeRecord>,
    self_baseline: Option<&ProjectNodeRecord>,
    use_declared_overlay_delta: bool,
) -> Result<Option<serde_json::Value>, ProjectPersistenceError> {
    if use_declared_overlay_delta {
        return project_persisted_data_from_runtime(node, matched_parent_baseline);
    }

    if matched_parent_baseline.is_some() {
        return raw_project_data_from_runtime(node);
    }

    project_persisted_data_from_runtime(node, self_baseline)
}

fn project_persisted_data_from_runtime<T: Node>(
    node: &T,
    baseline: Option<&ProjectNodeRecord>,
) -> Result<Option<serde_json::Value>, ProjectPersistenceError> {
    let node_type = node.get_type().to_string();
    let data_value = if let Some(parameter) = node.as_any().downcast_ref::<Parameter>() {
        let persist_runtime_value = baseline.is_none() || !parameter.read_only;
        let persist_constraints = baseline.is_none() || node.node_data().meta.user_permissions.can_edit_constraints;
        parameter.project_encode_data_against_baseline(
            baseline.and_then(|record| record.data.as_ref()),
            persist_runtime_value,
            persist_constraints,
        )
    } else {
        node.project_encode_data().map(|current| {
            diff_project_data_against_baseline(&current, baseline.and_then(|record| record.data.as_ref()))
        })
    }
    .map_err(|message| ProjectPersistenceError::Codec {
        node_type: node_type.clone(),
        message,
    })?;

    Ok((!data_value.is_null()).then_some(data_value))
}

fn raw_project_data_from_runtime<T: Node>(node: &T) -> Result<Option<serde_json::Value>, ProjectPersistenceError> {
    let node_type = node.get_type().to_string();
    let data_value = node
        .project_encode_data()
        .map_err(|message| ProjectPersistenceError::Codec {
            node_type: node_type.clone(),
            message,
        })?;

    Ok((!data_value.is_null()).then_some(data_value))
}

fn diff_project_data_against_baseline(
    current: &serde_json::Value,
    baseline: Option<&serde_json::Value>,
) -> serde_json::Value {
    let Some(baseline) = baseline else {
        return current.clone();
    };

    if current == baseline {
        return serde_json::Value::Null;
    }

    match (current.as_object(), baseline.as_object()) {
        (Some(current_object), Some(baseline_object)) => {
            if baseline_object.keys().any(|key| !current_object.contains_key(key)) {
                return current.clone();
            }

            let mut diff = serde_json::Map::new();
            for (key, value) in current_object {
                if baseline_object.get(key) != Some(value) {
                    diff.insert(key.clone(), value.clone());
                }
            }

            if diff.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Object(diff)
            }
        }
        _ => current.clone(),
    }
}

fn collect_referenced_uuids<T: Node>(engine: &Engine<T>) -> HashSet<NodeUuid> {
    let mut referenced_uuids = HashSet::<NodeUuid>::new();
    for (_, node) in engine.nodes.iter() {
        node.engine_visit_references(&mut |reference| {
            referenced_uuids.insert(reference.uuid());
        });
    }
    referenced_uuids
}

/// App-level runtime wrapper around a configured engine.
///
/// This is the intermediate layer where app concerns can grow later while the
/// engine remains responsible for runtime ticking/scheduling.
pub struct GoldenApp<T: Node> {
    engine: Engine<T>,
}

impl<T: Node> GoldenApp<T> {
    /// Creates an app wrapper around an engine instance.
    pub fn new(engine: Engine<T>) -> Self {
        Self { engine }
    }

    /// Returns a shared reference to the wrapped engine.
    pub fn engine(&self) -> &Engine<T> {
        &self.engine
    }

    /// Returns a mutable reference to the wrapped engine.
    pub fn engine_mut(&mut self) -> &mut Engine<T> {
        &mut self.engine
    }

    /// Consumes the wrapper and returns the wrapped engine.
    pub fn into_engine(self) -> Engine<T> {
        self.engine
    }

    /// Applies bootstrap edits, resolves scheduling, then enters the engine loop.
    pub fn run(mut self) -> Result<(), EngineRuntimeError> {
        self.engine.apply_edits()?;
        self.engine.run_pending_node_ready_callbacks()?;
        self.engine.resolve_if_needed()?;
        self.engine.clear_history();
        self.engine.run_loop()
    }
}

#[cfg(test)]
mod app_tests;
