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
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::node::{Node, NodeCreationContext, NodeReference, PotentialNodeHandle};
    use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, RangeConstraint};

    static PROJECT_LOAD_READY_CALLED: AtomicBool = AtomicBool::new(false);

    #[crate::node("lifecycle_marker_node", label = "Marker")]
    struct LifecycleMarkerNode {}

    #[crate::node("lifecycle_marker_node", from_struct)]
    impl Node for LifecycleMarkerNode {}

    #[crate::node("auto_loaded_node")]
    struct AutoLoadedNode {}

    #[crate::node("auto_loaded_node", from_struct)]
    impl Node for AutoLoadedNode {}

    #[crate::node("defaulted_loaded_node")]
    #[defaults(flag = true)]
    struct DefaultedLoadedNode {
        flag: bool,
    }

    #[crate::node("defaulted_loaded_node", from_struct)]
    impl Node for DefaultedLoadedNode {}

    #[crate::node("persisted_state_node")]
    struct PersistedStateNode {
        #[state(default = true, persist)]
        flag: bool,
    }

    #[crate::node("persisted_state_node", from_struct)]
    impl Node for PersistedStateNode {}

    #[crate::node("loaded_init_node")]
    #[defaults(init_calls = 0)]
    struct LoadedInitNode {
        init_calls: usize,
    }

    #[crate::node("loaded_init_node", from_struct)]
    impl Node for LoadedInitNode {
        fn init(&mut self, _ctx: &mut crate::process_ctx::ProcessCtx) {
            self.init_calls += 1;
        }
    }

    #[crate::node("loaded_ready_node")]
    struct LoadedReadyNode {}

    #[crate::node("loaded_ready_node", from_struct)]
    impl Node for LoadedReadyNode {
        fn on_node_ready(&mut self, _ctx: &mut crate::process_ctx::ProcessCtx, context: NodeCreationContext) {
            if context == NodeCreationContext::ProjectLoad {
                PROJECT_LOAD_READY_CALLED.store(true, Ordering::SeqCst);
            }
        }
    }

    #[crate::node("loaded_children_node")]
    #[children(
        value: f64 = 1.0 (
            label = "Value"
        );
    )]
    struct LoadedChildrenNode {}

    #[crate::node("loaded_children_node", from_struct)]
    impl Node for LoadedChildrenNode {}

    #[crate::node("loaded_nested_children_node")]
    #[children(
        folder(parameters, label = "Parameters", reuse = true) {
            port: String = "COM1" (label = "Port");

            folder(connection, label = "Connection") {
                baud_rate: i32 = 9600 (label = "Baud Rate");
            }

            folder(empty_group, label = "Empty Group") {}
        }
    )]
    struct LoadedNestedChildrenNode {}

    #[crate::node("loaded_nested_children_node", from_struct)]
    impl Node for LoadedNestedChildrenNode {}

    #[crate::node("loaded_potential_folder_node")]
    #[defaults(saw_parameters_in_init = false)]
    #[children(
        folder(parameters, label = "Parameters") {
            enabled: bool = true (label = "Enabled");
        }
    )]
    struct LoadedPotentialFolderNode {
        #[potential_node(decl_id = "parameters")]
        parameters: PotentialNodeHandle,
        saw_parameters_in_init: bool,
    }

    #[crate::node("loaded_potential_folder_node", from_struct)]
    impl Node for LoadedPotentialFolderNode {
        fn init(&mut self, _ctx: &mut crate::process_ctx::ProcessCtx) {
            self.saw_parameters_in_init = self.parameters.current_id().is_some();
        }
    }

    #[crate::node("sparse_noise_node")]
    #[children(
        runtime_flag: bool = true (
            label = "Runtime Flag",
            description = "Runtime-only flag",
            read_only = true
        );
        editable_flag: bool = true (
            label = "Editable Flag",
            description = "Editable flag"
        );
        dynamic_port: i32 = 1 (
            label = "Dynamic Port",
            description = "Dynamic port"
        );
        folder(receiver, label = "Receiver") {}
    )]
    struct SparseNoiseNode {}

    #[crate::node("sparse_noise_node", from_struct)]
    impl Node for SparseNoiseNode {}

    #[crate::node("via_persisted_state_base_node")]
    struct ViaPersistedStateBaseNode {
        #[state(default = true, persist)]
        base_flag: bool,
    }

    #[crate::node("via_persisted_state_base_node", from_struct)]
    impl Node for ViaPersistedStateBaseNode {}

    #[crate::node("via_persisted_state_wrapper_node")]
    struct ViaPersistedStateWrapperNode {
        base: ViaPersistedStateBaseNode,
        #[state(default = false, persist)]
        wrapper_flag: bool,
    }

    #[crate::node("via_persisted_state_wrapper_node", via = base, from_struct)]
    impl Node for ViaPersistedStateWrapperNode {
        fn project_create(node_type: &str) -> Option<Self> {
            (node_type == "via_persisted_state_wrapper_node").then(|| Self::new(ViaPersistedStateBaseNode::new()))
        }
    }

    #[crate::node("managed_item_manager_node", label = "Manager")]
    struct ManagedItemManagerNode {}

    #[crate::node("managed_item_manager_node", from_struct)]
    impl Node for ManagedItemManagerNode {
        crate::define_user_item_factory_methods! {
            accepts = ["managed_item", "managed_declared_item"];
            items = [
                {
                    type: ManagedItemNode,
                },
                {
                    type: ManagedDeclaredItemNode,
                },
            ];
        }
    }

    #[crate::node("managed_item_base_node")]
    struct ManagedItemBaseNode {}

    #[crate::node("managed_item_base_node", from_struct)]
    impl Node for ManagedItemBaseNode {}

    #[crate::node("managed_item_node", label = "Managed Item")]
    struct ManagedItemNode {
        base: ManagedItemBaseNode,
    }

    impl ManagedItemNode {
        fn create() -> Self {
            Self::new(ManagedItemBaseNode::new())
        }
    }

    #[crate::item("managed_item", node = "managed_item_node", via = base, from_struct)]
    impl Node for ManagedItemNode {}

    #[crate::node("managed_declared_item_node", label = "Managed Declared Item")]
    #[children(
        folder(settings, label = "Settings") {
            port: i32 = 9000 [0..65535] (
                label = "Port",
                widget = "text"
            );
            enabled: bool = true (
                label = "Enabled"
            );
        }
    )]
    struct ManagedDeclaredItemNode {}

    #[crate::item("managed_declared_item", node = "managed_declared_item_node", from_struct)]
    impl Node for ManagedDeclaredItemNode {}

    crate::define_node_enum!(
        enum LifecycleTestAppNode {
            LifecycleMarkerNode,
        }
    );

    crate::define_node_enum!(
        enum ProjectDecodeTestAppNode {
            AutoLoadedNode,
            LoadedChildrenNode,
            LoadedNestedChildrenNode,
            LoadedPotentialFolderNode,
            SparseNoiseNode,
            DefaultedLoadedNode,
            LoadedInitNode,
            LoadedReadyNode,
            PersistedStateNode,
            ViaPersistedStateBaseNode,
            ViaPersistedStateWrapperNode,
            ManagedItemManagerNode,
            ManagedItemBaseNode,
            ManagedItemNode,
            ManagedDeclaredItemNode,
        }
    );

    impl ProjectLifecycle for LifecycleTestAppNode {
        fn configure_engine(engine: &mut Engine<Self>) -> Result<(), String> {
            engine.set_ui_event_log_capacity(42);
            Ok(())
        }

        fn initialize_new_project(engine: &mut Engine<Self>) -> Result<(), String> {
            add_default_project_nodes(engine);
            engine.add_node(LifecycleMarkerNode::new().into(), None);
            Ok(())
        }

        fn project_opened(engine: &mut Engine<Self>) -> Result<(), String> {
            engine.set_ui_event_log_capacity(7);
            Ok(())
        }
    }

    #[test]
    fn create_new_project_engine_runs_app_lifecycle_hooks() {
        let mut engine =
            create_new_project_engine::<LifecycleTestAppNode>().expect("new project engine should be created");
        assert_eq!(
            engine.ui_event_log_capacity(),
            42,
            "configure_engine should run during new-project creation"
        );

        prepare_engine_for_runtime(&mut engine).expect("new project engine should prepare");

        let first_child = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("dashboard should exist under root");
        let second_child = engine
            .nodes
            .get(first_child)
            .and_then(|node| node.node_data().next_sibling)
            .expect("marker node should exist under root");

        assert_eq!(
            engine.nodes.get(first_child).map(Node::get_type),
            Some(crate::node::DASHBOARD_NODE_TYPE)
        );
        assert_eq!(
            engine.nodes.get(second_child).map(Node::get_type),
            Some("lifecycle_marker_node")
        );
    }

    #[test]
    fn configure_loaded_engine_reapplies_runtime_setup_after_deserialize() {
        let mut engine =
            create_new_project_engine::<LifecycleTestAppNode>().expect("new project engine should be created");
        prepare_engine_for_runtime(&mut engine).expect("new project engine should prepare");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let mut loaded = Engine::<LifecycleTestAppNode>::from_project_json_with(
            &json,
            <LifecycleTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        assert_eq!(
            loaded.ui_event_log_capacity(),
            8192,
            "deserialized engine should start with default runtime settings"
        );

        configure_loaded_engine(&mut loaded).expect("loaded engine should accept runtime configuration");
        assert_eq!(
            loaded.ui_event_log_capacity(),
            7,
            "project_opened should run after loading"
        );
    }

    #[test]
    fn from_struct_nodes_without_special_ctor_decode_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(AutoLoadedNode::new().into(), None);
        engine.apply_edits().expect("simple node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let simple = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("simple node should be restored under root");
        assert_eq!(loaded.nodes.get(simple).map(Node::get_type), Some("auto_loaded_node"));
    }

    #[test]
    fn defaulted_struct_fields_feed_generated_new_and_project_create() {
        let node = DefaultedLoadedNode::new();
        assert!(node.flag, "generated constructor should apply #[defaults(...)] values");

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(node.into(), None);
        engine.apply_edits().expect("defaulted node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("defaulted node should be restored under root");
        let ProjectDecodeTestAppNode::DefaultedLoadedNode(node) =
            loaded.nodes.get(restored).expect("defaulted node should exist")
        else {
            panic!("restored node should be a DefaultedLoadedNode");
        };

        assert!(
            node.flag,
            "autoloaded node should preserve generated default-backed constructor state"
        );
    }

    #[test]
    fn deserialized_nodes_run_init_after_project_decode() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedInitNode::new().into(), None);
        engine.apply_edits().expect("loaded-init node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-init node should be restored under root");
        let ProjectDecodeTestAppNode::LoadedInitNode(node) =
            loaded.nodes.get(restored).expect("loaded-init node should exist")
        else {
            panic!("restored node should be a LoadedInitNode");
        };

        assert_eq!(
            node.init_calls, 1,
            "deserialized nodes should replay init once after project decode"
        );
    }

    #[test]
    fn deserialized_nodes_defer_ready_until_runtime_prepare() {
        PROJECT_LOAD_READY_CALLED.store(false, Ordering::SeqCst);

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedReadyNode::new().into(), None);
        engine.apply_edits().expect("loaded-ready node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let mut loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        assert!(
            !PROJECT_LOAD_READY_CALLED.load(Ordering::SeqCst),
            "project deserialization should not run node-ready callbacks"
        );

        prepare_engine_for_runtime(&mut loaded).expect("loaded project should prepare");
        assert!(
            PROJECT_LOAD_READY_CALLED.load(Ordering::SeqCst),
            "runtime preparation should drain deferred node-ready callbacks"
        );
    }

    #[test]
    fn deserialized_nodes_do_not_duplicate_declared_children() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedChildrenNode::new().into(), None);
        engine.apply_edits().expect("loaded-children node should attach");

        let source = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-children node should exist under root");
        assert_eq!(
            count_direct_children(&engine, source),
            1,
            "source node should materialize one declared child"
        );

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-children node should be restored under root");
        assert_eq!(
            count_direct_children(&loaded, restored),
            1,
            "deserialized node should not duplicate declared children"
        );
    }

    #[test]
    fn duplicated_subtrees_do_not_duplicate_declared_children() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedChildrenNode::new().into(), None);
        engine.apply_edits().expect("loaded-children node should attach");

        let source = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-children node should exist under root");
        let duplicated = engine
            .duplicate_subtree_with(
                source,
                engine.root,
                Some(source),
                Some("Loaded Children Copy".to_string()),
                |node| node.project_encode_data(),
                <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
            )
            .expect("loaded-children subtree should duplicate");

        assert_eq!(
            count_direct_children(&engine, duplicated),
            1,
            "duplicated subtree should not duplicate declared children"
        );
    }

    #[test]
    fn deserialized_nodes_do_not_duplicate_nested_declared_children() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedNestedChildrenNode::new().into(), None);
        engine.apply_edits().expect("loaded-nested-children node should attach");

        let source = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-nested-children node should exist under root");
        assert_nested_declared_child_shape(&engine, source);

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-nested-children node should be restored under root");
        assert_nested_declared_child_shape(&loaded, restored);
    }

    #[test]
    fn duplicated_subtrees_do_not_duplicate_nested_declared_children() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedNestedChildrenNode::new().into(), None);
        engine.apply_edits().expect("loaded-nested-children node should attach");

        let source = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-nested-children node should exist under root");
        let duplicated = engine
            .duplicate_subtree_with(
                source,
                engine.root,
                Some(source),
                Some("Loaded Nested Children Copy".to_string()),
                |node| node.project_encode_data(),
                <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
            )
            .expect("loaded-nested-children subtree should duplicate");

        assert_nested_declared_child_shape(&engine, duplicated);
    }

    #[test]
    fn deserialized_nodes_bind_potential_folder_handles_before_init() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedPotentialFolderNode::new().into(), None);
        engine
            .apply_edits()
            .expect("loaded-potential-folder node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-potential-folder node should be restored under root");
        let ProjectDecodeTestAppNode::LoadedPotentialFolderNode(node) = loaded
            .nodes
            .get(restored)
            .expect("loaded-potential-folder node should exist")
        else {
            panic!("restored node should be a LoadedPotentialFolderNode");
        };

        assert!(
            node.parameters.current_id().is_some(),
            "loaded potential folder handle should bind to the existing declared folder"
        );
        assert!(
            node.saw_parameters_in_init,
            "loaded potential folder handle should already be bound when init runs"
        );
    }

    #[test]
    fn duplicated_subtrees_bind_potential_folder_handles_before_init() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedPotentialFolderNode::new().into(), None);
        engine
            .apply_edits()
            .expect("loaded-potential-folder node should attach");

        let source = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-potential-folder node should exist under root");
        let duplicated = engine
            .duplicate_subtree_with(
                source,
                engine.root,
                Some(source),
                Some("Loaded Potential Folder Copy".to_string()),
                |node| node.project_encode_data(),
                <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
            )
            .expect("loaded-potential-folder subtree should duplicate");

        let ProjectDecodeTestAppNode::LoadedPotentialFolderNode(node) = engine
            .nodes
            .get(duplicated)
            .expect("duplicated loaded-potential-folder node should exist")
        else {
            panic!("duplicated node should be a LoadedPotentialFolderNode");
        };

        assert!(
            node.parameters.current_id().is_some(),
            "duplicated potential folder handle should bind to the existing declared folder"
        );
        assert!(
            node.saw_parameters_in_init,
            "duplicated potential folder handle should already be bound when init runs"
        );
    }

    fn count_direct_children(engine: &Engine<ProjectDecodeTestAppNode>, node_id: crate::node::NodeId) -> usize {
        let mut count = 0usize;
        let mut child = engine.nodes.get(node_id).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            count = count.saturating_add(1);
            child = engine
                .nodes
                .get(child_id)
                .and_then(|node| node.node_data().next_sibling);
        }
        count
    }

    fn direct_child_decl_ids(engine: &Engine<ProjectDecodeTestAppNode>, node_id: crate::node::NodeId) -> Vec<String> {
        let mut decl_ids = Vec::new();
        let mut child = engine.nodes.get(node_id).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let Some(child_node) = engine.nodes.get(child_id) else {
                break;
            };
            decl_ids.push(child_node.node_data().meta.decl_id.0.clone());
            child = child_node.node_data().next_sibling;
        }
        decl_ids
    }

    fn assert_nested_declared_child_shape(engine: &Engine<ProjectDecodeTestAppNode>, owner_id: crate::node::NodeId) {
        let parameters =
            find_direct_child_by_decl(engine, owner_id, "parameters").expect("parameters folder should exist");
        assert_eq!(
            count_direct_children(engine, owner_id),
            1,
            "owner should only have one top-level declared folder"
        );
        assert_eq!(
            count_direct_children(engine, parameters),
            3,
            "parameters folder should keep exactly its declared children"
        );
        assert_eq!(count_direct_decl(engine, owner_id, "parameters"), 1);
        assert_eq!(count_direct_decl(engine, parameters, "parameters/port"), 1);
        assert_eq!(count_direct_decl(engine, parameters, "parameters/connection"), 1);
        assert_eq!(count_direct_decl(engine, parameters, "parameters/empty_group"), 1);

        let connection = find_direct_child_by_decl(engine, parameters, "parameters/connection")
            .expect("connection folder should exist");
        assert_eq!(count_direct_children(engine, connection), 1);
        assert_eq!(
            count_direct_decl(engine, connection, "parameters/connection/baud_rate"),
            1
        );

        let empty_group = find_direct_child_by_decl(engine, parameters, "parameters/empty_group")
            .expect("empty group folder should exist");
        assert_eq!(
            count_direct_children(engine, empty_group),
            0,
            "empty group should stay empty"
        );
    }

    fn find_direct_child_by_decl(
        engine: &Engine<ProjectDecodeTestAppNode>,
        node_id: crate::node::NodeId,
        decl_id: &str,
    ) -> Option<crate::node::NodeId> {
        let mut child = engine.nodes.get(node_id).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id)?;
            if child_node.node_data().meta.decl_id.0 == decl_id {
                return Some(child_id);
            }
            child = child_node.node_data().next_sibling;
        }
        None
    }

    fn count_direct_decl(
        engine: &Engine<ProjectDecodeTestAppNode>,
        node_id: crate::node::NodeId,
        decl_id: &str,
    ) -> usize {
        let mut count = 0usize;
        let mut child = engine.nodes.get(node_id).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let Some(child_node) = engine.nodes.get(child_id) else {
                break;
            };
            if child_node.node_data().meta.decl_id.0 == decl_id {
                count = count.saturating_add(1);
            }
            child = child_node.node_data().next_sibling;
        }
        count
    }

    fn find_decl_path(
        engine: &Engine<ProjectDecodeTestAppNode>,
        start: crate::node::NodeId,
        path: &str,
    ) -> Option<crate::node::NodeId> {
        let mut current = start;
        let mut accumulated = Vec::<&str>::new();
        for segment in path.trim_matches('/').split('/').filter(|segment| !segment.is_empty()) {
            accumulated.push(segment);
            let decl_id = accumulated.join("/");
            current = find_direct_child_by_decl(engine, current, decl_id.as_str())?;
        }
        Some(current)
    }

    fn json_child_by_decl<'a>(record: &'a serde_json::Value, decl_id: &str) -> &'a serde_json::Value {
        record
            .get("children")
            .and_then(serde_json::Value::as_array)
            .and_then(|children| {
                children.iter().find(|child| {
                    child
                        .get("meta")
                        .and_then(|meta| meta.get("decl_id"))
                        .and_then(serde_json::Value::as_str)
                        == Some(decl_id)
                })
            })
            .unwrap_or_else(|| panic!("child record '{decl_id}' should exist"))
    }

    #[test]
    fn state_fields_can_define_default_and_persistence_in_one_place() {
        let mut node = PersistedStateNode::new();
        assert!(
            node.flag,
            "generated constructor should apply #[state(default = ...)] values"
        );
        node.flag = false;

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(node.into(), None);
        engine.apply_edits().expect("persisted-state node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("persisted-state node should be restored under root");
        let ProjectDecodeTestAppNode::PersistedStateNode(node) =
            loaded.nodes.get(restored).expect("persisted-state node should exist")
        else {
            panic!("restored node should be a PersistedStateNode");
        };

        assert!(
            !node.flag,
            "persisted #[state(..., persist)] field should round-trip through project save/load"
        );
    }

    #[test]
    fn via_nodes_can_persist_wrapper_and_base_state_without_manual_codecs() {
        let mut node = ViaPersistedStateWrapperNode::new(ViaPersistedStateBaseNode::new());
        assert!(
            node.base.base_flag,
            "via base should use generated default-backed constructor"
        );
        assert!(
            !node.wrapper_flag,
            "via wrapper should use generated default-backed constructor"
        );

        node.base.base_flag = false;
        node.wrapper_flag = true;

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(node.into(), None);
        engine.apply_edits().expect("via persisted-state node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("via persisted-state node should be restored under root");
        let ProjectDecodeTestAppNode::ViaPersistedStateWrapperNode(node) = loaded
            .nodes
            .get(restored)
            .expect("via persisted-state node should exist")
        else {
            panic!("restored node should be a ViaPersistedStateWrapperNode");
        };

        assert!(
            !node.base.base_flag,
            "via base persisted state should round-trip through project save/load"
        );
        assert!(
            node.wrapper_flag,
            "via wrapper persisted state should round-trip through project save/load"
        );
    }

    #[test]
    fn managed_item_nodes_decode_from_parent_factory_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new().into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create().into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let loaded_manager = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should be restored");
        let loaded_item = loaded
            .nodes
            .get(loaded_manager)
            .and_then(|node| node.node_data().first_child)
            .expect("managed item should be restored");

        assert_eq!(
            loaded.nodes.get(loaded_item).map(Node::get_type),
            Some("managed_item_node")
        );
        assert_eq!(
            loaded
                .nodes
                .get(loaded_item)
                .expect("managed item should exist")
                .node_data()
                .user_role,
            crate::node::UserNodeRole::ItemRoot
        );
    }

    #[test]
    fn managed_item_nodes_duplicate_from_parent_factory_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new().into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create().into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let item = engine
            .nodes
            .get(manager)
            .and_then(|node| node.node_data().first_child)
            .expect("managed item should exist under manager");
        let duplicated = engine
            .duplicate_subtree_with(
                item,
                manager,
                Some(item),
                Some("Item Copy".to_string()),
                |node| node.project_encode_data(),
                <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
            )
            .expect("managed item should duplicate");

        assert_eq!(
            engine.nodes.get(duplicated).map(Node::get_type),
            Some("managed_item_node")
        );
        assert_eq!(
            engine
                .nodes
                .get(duplicated)
                .expect("duplicated managed item should exist")
                .node_data()
                .meta
                .label,
            "Item Copy"
        );
        assert_eq!(
            engine
                .nodes
                .get(duplicated)
                .expect("duplicated managed item should exist")
                .node_data()
                .user_role,
            crate::node::UserNodeRole::ItemRoot
        );
    }

    #[test]
    fn sparse_project_serialization_omits_default_declared_children() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedChildrenNode::new().into(), None);
        engine.apply_edits().expect("loaded-children node should attach");

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");

        let project_children = value
            .get("root")
            .and_then(|root| root.get("children"))
            .and_then(serde_json::Value::as_array)
            .expect("root should contain one child record");
        let node_record = project_children.first().expect("loaded-children record should exist");

        assert!(
            node_record.get("children").is_none(),
            "default declared children should be omitted from sparse project output"
        );

        let loaded = from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-children node should be restored under root");
        assert_eq!(
            count_direct_children(&loaded, restored),
            1,
            "omitted declared children should be recreated during load"
        );
    }

    #[test]
    fn sparse_project_serialization_keeps_referenced_default_children() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(LoadedChildrenNode::new().into(), None);
        engine.apply_edits().expect("loaded-children node should attach");

        let owner = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("loaded-children node should exist under root");
        let target = engine
            .nodes
            .get(owner)
            .and_then(|node| node.node_data().first_child)
            .expect("default child should exist");
        let target_uuid = engine
            .nodes
            .get(target)
            .expect("default child should exist")
            .node_data()
            .meta
            .uuid;

        engine.add_node(
            Parameter::new(
                "Target Ref",
                ParamValue::Reference(NodeReference::new(target_uuid)),
                ParameterChangeCheck::ValueChange,
            )
            .into(),
            Some(owner),
        );
        engine.apply_edits().expect("reference parameter should attach");

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
        let project_children = value
            .get("root")
            .and_then(|root| root.get("children"))
            .and_then(serde_json::Value::as_array)
            .expect("root should contain the owner node");
        let owner_record = project_children.first().expect("owner record should exist");
        let owner_children = owner_record
            .get("children")
            .and_then(serde_json::Value::as_array)
            .expect("owner should keep child records when one is referenced");

        assert!(
            owner_children
                .iter()
                .any(|child| child.get("type") == Some(&serde_json::Value::String("float".to_string()))),
            "referenced default child should stay in sparse project output to preserve its uuid"
        );

        let loaded = from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");
        let loaded_owner = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("owner should reload");
        let loaded_target = loaded
            .nodes
            .get(loaded_owner)
            .and_then(|node| node.node_data().first_child)
            .expect("target child should reload");
        let loaded_ref = loaded
            .nodes
            .get(loaded_target)
            .and_then(|node| node.node_data().next_sibling)
            .expect("reference child should reload");

        let ProjectDecodeTestAppNode::Parameter(reference_node) =
            loaded.nodes.get(loaded_ref).expect("reference node should exist")
        else {
            panic!("reference node should decode as a Parameter");
        };

        match &reference_node.value {
            ParamValue::Reference(reference) => {
                assert_eq!(reference.cached_id(), Some(loaded_target));
            }
            other => panic!("expected reference parameter after reload, got {other:?}"),
        }
    }

    #[test]
    fn sparse_project_serialization_omits_default_generated_state_payloads() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(PersistedStateNode::new().into(), None);
        engine.apply_edits().expect("persisted-state node should attach");

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
        let node_record = value
            .get("root")
            .and_then(|root| root.get("children"))
            .and_then(serde_json::Value::as_array)
            .and_then(|children| children.first())
            .expect("persisted-state node should be saved");

        assert!(
            node_record.get("data").is_none(),
            "default persisted-state fields should be omitted from sparse project output"
        );
    }

    #[test]
    fn sparse_project_serialization_omits_runtime_parameter_noise_and_warnings() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(SparseNoiseNode::new().into(), None);
        engine.apply_edits().expect("sparse-noise node should attach");

        let owner = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("sparse-noise node should exist under root");
        let runtime_flag =
            find_direct_child_by_decl(&engine, owner, "runtime_flag").expect("runtime-flag child should exist");
        let dynamic_port =
            find_direct_child_by_decl(&engine, owner, "dynamic_port").expect("dynamic-port child should exist");
        let receiver = find_direct_child_by_decl(&engine, owner, "receiver").expect("receiver folder should exist");

        let _ = engine
            .nodes
            .get_mut(runtime_flag)
            .expect("runtime-flag child should exist")
            .engine_set_param_value(ParamValue::Bool(false));
        engine
            .nodes
            .get_mut(dynamic_port)
            .expect("dynamic-port child should exist")
            .engine_set_param_constraints(ParameterConstraints {
                range: RangeConstraint::uniform(Some(0.0), Some(65535.0)),
                ..Default::default()
            })
            .expect("dynamic-port constraints should update");
        engine
            .nodes
            .get_mut(receiver)
            .expect("receiver folder should exist")
            .node_data_mut()
            .meta
            .set_warning(Some("runtime"), "runtime warning", None);

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
        let owner_record = value
            .get("root")
            .and_then(|root| root.get("children"))
            .and_then(serde_json::Value::as_array)
            .and_then(|children| children.first())
            .expect("sparse-noise node should be saved");

        assert!(
            owner_record.get("children").is_none(),
            "runtime-only parameter value, dynamic constraints, and warnings should not force default children into sparse output"
        );

        let loaded = from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");
        let restored_owner = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("sparse-noise node should reload");

        assert_eq!(
            count_direct_children(&loaded, restored_owner),
            4,
            "omitted default children should still be recreated during load"
        );

        let restored_runtime_flag = find_direct_child_by_decl(&loaded, restored_owner, "runtime_flag")
            .expect("runtime-flag child should reload");
        let ProjectDecodeTestAppNode::Parameter(runtime_flag) = loaded
            .nodes
            .get(restored_runtime_flag)
            .expect("runtime-flag child should exist")
        else {
            panic!("runtime-flag child should decode as a Parameter");
        };
        assert_eq!(
            runtime_flag.value,
            ParamValue::Bool(true),
            "read-only runtime value changes should not persist into sparse project files"
        );

        let restored_dynamic_port = find_direct_child_by_decl(&loaded, restored_owner, "dynamic_port")
            .expect("dynamic-port child should reload");
        let ProjectDecodeTestAppNode::Parameter(dynamic_port) = loaded
            .nodes
            .get(restored_dynamic_port)
            .expect("dynamic-port child should exist")
        else {
            panic!("dynamic-port child should decode as a Parameter");
        };
        assert_eq!(
            dynamic_port.constraints,
            ParameterConstraints::default(),
            "non-editable runtime constraint changes should not persist into sparse project files"
        );

        let restored_receiver =
            find_direct_child_by_decl(&loaded, restored_owner, "receiver").expect("receiver folder should reload");
        assert!(
            loaded
                .nodes
                .get(restored_receiver)
                .expect("receiver folder should exist")
                .node_data()
                .meta
                .presentation
                .warnings
                .is_empty(),
            "runtime warnings should not persist into sparse project files"
        );
    }

    #[test]
    fn sparse_project_serialization_keeps_changed_editable_parameter_without_doc_fields() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(SparseNoiseNode::new().into(), None);
        engine.apply_edits().expect("sparse-noise node should attach");

        let owner = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("sparse-noise node should exist under root");
        let editable_flag =
            find_direct_child_by_decl(&engine, owner, "editable_flag").expect("editable-flag child should exist");
        let _ = engine
            .nodes
            .get_mut(editable_flag)
            .expect("editable-flag child should exist")
            .engine_set_param_value(ParamValue::Bool(false));

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
        let owner_record = value
            .get("root")
            .and_then(|root| root.get("children"))
            .and_then(serde_json::Value::as_array)
            .and_then(|children| children.first())
            .expect("sparse-noise node should be saved");
        let owner_children = owner_record
            .get("children")
            .and_then(serde_json::Value::as_array)
            .expect("changed editable child should stay in sparse output");

        assert_eq!(
            owner_children.len(),
            1,
            "only the changed editable child should be persisted"
        );
        let child_record = owner_children.first().expect("editable child record should exist");
        let child_meta = child_record
            .get("meta")
            .and_then(serde_json::Value::as_object)
            .expect("editable child should keep metadata needed for reload");

        assert!(
            child_meta.get("description").is_none(),
            "default descriptions should be omitted from sparse project output"
        );
        assert!(
            child_meta.get("declared_description_key").is_none(),
            "declaration description keys should be omitted from sparse project output"
        );
        assert!(
            child_meta.get("declared_description").is_none(),
            "declared descriptions should be omitted from sparse project output"
        );

        let loaded = from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");
        let restored_owner = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("sparse-noise node should reload");
        let restored_editable_flag = find_direct_child_by_decl(&loaded, restored_owner, "editable_flag")
            .expect("editable-flag child should reload");
        let ProjectDecodeTestAppNode::Parameter(editable_flag) = loaded
            .nodes
            .get(restored_editable_flag)
            .expect("editable-flag child should exist")
        else {
            panic!("editable-flag child should decode as a Parameter");
        };

        assert_eq!(
            editable_flag.value,
            ParamValue::Bool(false),
            "editable parameter changes should still round-trip through sparse project files"
        );
    }

    #[test]
    fn sparse_project_load_preserves_declared_child_order_around_saved_deltas() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(SparseNoiseNode::new().into(), None);
        engine.add_node(LoadedNestedChildrenNode::new().into(), None);
        engine.apply_edits().expect("declared nodes should attach");

        let sparse_noise = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("sparse-noise node should exist under root");
        let nested = engine
            .nodes
            .get(sparse_noise)
            .and_then(|node| node.node_data().next_sibling)
            .expect("loaded-nested node should exist under root");

        let editable_flag =
            find_direct_child_by_decl(&engine, sparse_noise, "editable_flag").expect("editable flag should exist");
        let _ = engine
            .nodes
            .get_mut(editable_flag)
            .expect("editable flag should exist")
            .engine_set_param_value(ParamValue::Bool(false));

        let baud_rate = find_decl_path(&engine, nested, "parameters/connection/baud_rate")
            .expect("baud rate parameter should exist");
        let _ = engine
            .nodes
            .get_mut(baud_rate)
            .expect("baud rate parameter should exist")
            .engine_set_param_value(ParamValue::Int(115200));

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let loaded = from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");

        let loaded_sparse_noise = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("sparse-noise node should reload");
        assert_eq!(
            direct_child_decl_ids(&loaded, loaded_sparse_noise),
            ["runtime_flag", "editable_flag", "dynamic_port", "receiver"],
            "saved declared child deltas should hydrate in their declared slot"
        );

        let loaded_nested = loaded
            .nodes
            .get(loaded_sparse_noise)
            .and_then(|node| node.node_data().next_sibling)
            .expect("loaded-nested node should reload");
        let parameters =
            find_direct_child_by_decl(&loaded, loaded_nested, "parameters").expect("parameters folder should reload");
        assert_eq!(
            direct_child_decl_ids(&loaded, parameters),
            ["parameters/port", "parameters/connection", "parameters/empty_group"],
            "saved nested folder deltas should not jump ahead of omitted declared siblings"
        );

        let loaded_baud_rate = find_decl_path(&loaded, loaded_nested, "parameters/connection/baud_rate")
            .expect("baud rate parameter should reload");
        let ProjectDecodeTestAppNode::Parameter(baud_rate) = loaded
            .nodes
            .get(loaded_baud_rate)
            .expect("baud rate parameter should exist")
        else {
            panic!("baud rate should decode as a Parameter");
        };
        assert_eq!(baud_rate.value, ParamValue::Int(115200));
    }

    #[test]
    fn sparse_project_serialization_is_idempotent_for_declared_item_child_deltas() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new().into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should exist under root");
        engine.add_user_item(ManagedDeclaredItemNode::new().into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");
        for _ in 0..4 {
            engine
                .apply_edits()
                .expect("managed item declared children should materialize");
        }

        let item = engine
            .nodes
            .get(manager)
            .and_then(|manager| manager.node_data().first_child)
            .expect("managed declared item should exist");
        let port = find_decl_path(&engine, item, "settings/port").expect("declared port should exist");
        let _ = engine
            .nodes
            .get_mut(port)
            .expect("declared port should exist")
            .engine_set_param_value(ParamValue::Int(9001));

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let round_tripped =
            from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");
        let round_tripped_json =
            to_sparse_project_json_pretty(&round_tripped).expect("round-tripped sparse project should encode");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&round_tripped_json)
                .expect("round-tripped project json should parse"),
            serde_json::from_str::<serde_json::Value>(&json).expect("project json should parse"),
            "generic declared item child deltas should be stable across save/load/save"
        );

        let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
        let item_record = value
            .get("root")
            .and_then(|root| root.get("children"))
            .and_then(serde_json::Value::as_array)
            .and_then(|children| children.first())
            .and_then(|manager| manager.get("children"))
            .and_then(serde_json::Value::as_array)
            .and_then(|children| children.first())
            .expect("managed declared item record should be saved");
        let settings_record = json_child_by_decl(item_record, "settings");
        let port_record = json_child_by_decl(settings_record, "settings/port");

        assert_eq!(
            settings_record.get("meta"),
            Some(&serde_json::json!({ "decl_id": "settings" })),
            "declared ancestor metadata should stay app-owned"
        );
        assert_eq!(
            port_record.get("meta"),
            Some(&serde_json::json!({ "decl_id": "settings/port" })),
            "declared parameter metadata should stay app-owned"
        );
        assert_eq!(
            port_record.get("data"),
            Some(&serde_json::json!({ "value": { "Int": 9001 } })),
            "only the changed declared parameter value should be persisted"
        );
    }

    #[test]
    fn sparse_project_serialization_keeps_managed_items() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new().into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create().into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let json = to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
        let loaded = from_sparse_project_json::<ProjectDecodeTestAppNode>(&json).expect("sparse project should decode");

        let loaded_manager = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should reload");
        let loaded_item = loaded
            .nodes
            .get(loaded_manager)
            .and_then(|node| node.node_data().first_child)
            .expect("managed item should reload");

        assert_eq!(
            loaded.nodes.get(loaded_item).map(Node::get_type),
            Some("managed_item_node")
        );
    }
}
