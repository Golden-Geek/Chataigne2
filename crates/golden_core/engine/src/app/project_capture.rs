use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::engine::{Engine, PROJECT_FILE_VERSION, ProjectFile, ProjectNodeRecord, ProjectPersistenceError};
use crate::node::{Node, NodeId, NodeMeta, NodeUuid, UserNodeRole};
use crate::parameter::Parameter;

use super::{
    Folder, ProjectNode, diff_project_data_against_baseline, node_is_app_data_persisted,
    project_meta_delta_from_runtime, project_meta_for_sparse_record, project_meta_from_runtime,
    raw_project_data_from_runtime, sparse_baseline_child_identity_count, sparse_json_cache_key,
};

const PROJECT_CAPTURE_SHARD_COUNT: usize = 256;
type ProjectNodeShard = Arc<HashMap<NodeId, Arc<CapturedProjectNode>>>;

/// One fully owned authored node used by immutable project captures.
pub(crate) struct CapturedProjectNode {
    pub(crate) node_id: NodeId,
    pub(crate) node_type: Arc<str>,
    pub(crate) user_role: UserNodeRole,
    pub(crate) meta: NodeMeta,
    pub(crate) data: Arc<serde_json::Value>,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Arc<[NodeId]>,
    pub(crate) referenced_uuids: Arc<[NodeUuid]>,
    pub(crate) app_data_persisted: bool,
}

impl CapturedProjectNode {
    /// Captures one node's authored representation and direct structure.
    pub(crate) fn from_engine<T: Node>(engine: &Engine<T>, node_id: NodeId) -> Result<Self, ProjectPersistenceError> {
        let node = engine
            .nodes
            .get(node_id)
            .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
        let node_type: Arc<str> = node.get_type().into();
        let data = node
            .project_encode_data()
            .map_err(|message| ProjectPersistenceError::Codec {
                node_type: node_type.to_string(),
                message,
            })?;
        let mut children = Vec::new();
        let mut child = node.node_data().first_child;
        while let Some(child_id) = child {
            children.push(child_id);
            child = engine
                .nodes
                .get(child_id)
                .ok_or(ProjectPersistenceError::MissingNode(child_id))?
                .node_data()
                .next_sibling;
        }
        let mut referenced_uuids = Vec::new();
        node.engine_visit_references(&mut |reference| referenced_uuids.push(reference.uuid()));
        referenced_uuids.sort_unstable_by_key(|uuid| uuid.0);
        referenced_uuids.dedup();

        Ok(Self {
            node_id,
            node_type,
            user_role: node.node_data().user_role,
            meta: node.node_data().meta.clone(),
            data: Arc::new(data),
            parent: node.node_data().parent,
            children: children.into(),
            referenced_uuids: referenced_uuids.into(),
            app_data_persisted: node_is_app_data_persisted(node),
        })
    }
}

/// Fixed-shard copy-on-write authored project graph.
#[derive(Clone)]
pub(crate) struct ProjectGraphCapture {
    root: NodeId,
    shards: Box<[ProjectNodeShard]>,
    len: usize,
}

impl ProjectGraphCapture {
    /// Captures a complete graph for initial publication or detached project replacement.
    pub(crate) fn from_engine<T: Node>(engine: &Engine<T>) -> Result<Self, ProjectPersistenceError> {
        let mut graph = Self::empty(engine.root);
        for (node_id, _) in engine.nodes.iter() {
            graph.insert(CapturedProjectNode::from_engine(engine, node_id)?);
        }
        Ok(graph)
    }

    fn empty(root: NodeId) -> Self {
        Self {
            root,
            shards: (0..PROJECT_CAPTURE_SHARD_COUNT)
                .map(|_| Arc::new(HashMap::new()))
                .collect(),
            len: 0,
        }
    }

    pub(crate) fn root(&self) -> NodeId {
        self.root
    }

    pub(crate) fn get(&self, node: &NodeId) -> Option<&CapturedProjectNode> {
        self.shards[shard_index(*node)].get(node).map(Arc::as_ref)
    }

    pub(crate) fn insert(&mut self, node: CapturedProjectNode) {
        let shard = Arc::make_mut(&mut self.shards[shard_index(node.node_id)]);
        if shard.insert(node.node_id, Arc::new(node)).is_none() {
            self.len += 1;
        }
    }

    pub(crate) fn remove(&mut self, node: &NodeId) {
        if Arc::make_mut(&mut self.shards[shard_index(*node)])
            .remove(node)
            .is_some()
        {
            self.len -= 1;
        }
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    fn values(&self) -> impl Iterator<Item = &CapturedProjectNode> {
        self.shards.iter().flat_map(|shard| shard.values().map(Arc::as_ref))
    }

    #[cfg(test)]
    pub(crate) fn shared_shards_with(&self, other: &Self) -> usize {
        self.shards
            .iter()
            .zip(other.shards.iter())
            .filter(|(left, right)| Arc::ptr_eq(left, right))
            .count()
    }

    #[cfg(test)]
    pub(crate) const fn shard_count() -> usize {
        PROJECT_CAPTURE_SHARD_COUNT
    }
}

/// Materializes the canonical sparse project document from an immutable authored graph.
pub(crate) fn sparse_project_file_from_capture<T>(
    graph: &ProjectGraphCapture,
    ui_state: Option<serde_json::Value>,
) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let referenced_uuids = graph
        .values()
        .flat_map(|node| node.referenced_uuids.iter().copied())
        .collect::<HashSet<_>>();
    let mut cache = CapturedSparseEncodeCache::default();
    let structural_root = cache.structural_baseline_for_node::<T>(graph, graph.root())?;
    let root = encode_sparse_node_record::<T>(
        graph,
        graph.root(),
        Some(&structural_root),
        false,
        true,
        &referenced_uuids,
        &mut cache,
    )?
    .ok_or_else(|| ProjectPersistenceError::Codec {
        node_type: graph
            .get(&graph.root())
            .map(|node| node.node_type.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        message: "root node cannot be omitted from sparse project output".to_string(),
    })?;

    Ok(ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state,
        root,
    })
}

#[derive(Default)]
struct CapturedSparseEncodeCache {
    structural_baselines: HashMap<String, ProjectNodeRecord>,
    default_baselines: HashMap<String, Option<ProjectNodeRecord>>,
}

impl CapturedSparseEncodeCache {
    fn structural_baseline_for_node<T>(
        &mut self,
        graph: &ProjectGraphCapture,
        node_id: NodeId,
    ) -> Result<ProjectNodeRecord, ProjectPersistenceError>
    where
        T: ProjectNode + From<Folder>,
    {
        let key = sparse_encode_baseline_cache_key(graph, node_id, true)?;
        if let Some(baseline) = self.structural_baselines.get(&key) {
            let mut baseline = baseline.clone();
            let node = required_node(graph, node_id)?;
            baseline.uuid = node.meta.uuid;
            baseline.meta = project_meta_from_runtime(&node.meta);
            return Ok(baseline);
        }

        let baseline = build_structural_baseline_record_for_node::<T>(graph, node_id)?;
        self.structural_baselines.insert(key, baseline.clone());
        Ok(baseline)
    }

    fn default_baseline_for_node<T>(
        &mut self,
        graph: &ProjectGraphCapture,
        node_id: NodeId,
    ) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
    where
        T: ProjectNode + From<Folder>,
    {
        let key = sparse_encode_baseline_cache_key(graph, node_id, false)?;
        if let Some(baseline) = self.default_baselines.get(&key) {
            return Ok(baseline.clone());
        }

        let baseline = build_default_baseline_record_for_node::<T>(graph, node_id)?;
        self.default_baselines.insert(key, baseline.clone());
        Ok(baseline)
    }
}

fn encode_sparse_node_record<T>(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
    matched_parent_baseline: Option<&ProjectNodeRecord>,
    allow_omission: bool,
    omit_app_data_nodes: bool,
    referenced_uuids: &HashSet<NodeUuid>,
    cache: &mut CapturedSparseEncodeCache,
) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let captured = required_node(graph, node_id)?;
    if omit_app_data_nodes && captured.app_data_persisted {
        return Ok(None);
    }
    let node = recreate_node_from_capture::<T>(graph, node_id)?;
    let matched_parent_baseline = matched_parent_baseline.filter(|baseline| {
        baseline.node_type == captured.node_type.as_ref() && baseline.user_role == captured.user_role
    });
    let self_matches_parent_baseline = if let Some(baseline) = matched_parent_baseline {
        project_meta_delta_from_runtime(&captured.meta, Some(&baseline.meta)).is_empty()
            && project_persisted_data_from_runtime_capture(&node, Some(baseline))?.is_none()
    } else {
        false
    };

    let structural_baseline_storage;
    let baseline_children = if self_matches_parent_baseline {
        matched_parent_baseline.map_or(&[][..], |baseline| baseline.children.as_slice())
    } else {
        structural_baseline_storage = Some(cache.structural_baseline_for_node::<T>(graph, node_id)?);
        structural_baseline_storage
            .as_ref()
            .map_or(&[][..], |baseline| baseline.children.as_slice())
    };
    let default_baseline_storage;
    let self_baseline = if let Some(baseline) = matched_parent_baseline {
        Some(baseline)
    } else {
        default_baseline_storage = cache.default_baseline_for_node::<T>(graph, node_id)?;
        default_baseline_storage.as_ref()
    };

    let mut children = Vec::new();
    for child_id in captured.children.iter().copied() {
        let child = required_node(graph, child_id)?;
        let matched_child_baseline = find_matching_child_baseline(child, baseline_children);
        let repeated_child_baseline = matched_child_baseline
            .is_some_and(|baseline| sparse_baseline_child_identity_count(baseline_children, baseline) > 1);
        let child_allow_omission = matched_child_baseline.is_some() && !repeated_child_baseline;
        if let Some(record) = encode_sparse_node_record::<T>(
            graph,
            child_id,
            matched_child_baseline,
            child_allow_omission,
            omit_app_data_nodes,
            referenced_uuids,
            cache,
        )? {
            children.push(record);
        }
    }

    if allow_omission
        && self_matches_parent_baseline
        && children.is_empty()
        && !referenced_uuids.contains(&captured.meta.uuid)
    {
        return Ok(None);
    }
    let meta = project_meta_for_sparse_record(
        &captured.meta,
        matched_parent_baseline.map(|baseline| &baseline.meta),
        self_baseline.map(|baseline| &baseline.meta),
        allow_omission && matched_parent_baseline.is_some(),
    );
    let data = project_data_for_sparse_record_capture(
        &node,
        matched_parent_baseline,
        self_baseline,
        allow_omission && matched_parent_baseline.is_some(),
    )?;

    Ok(Some(ProjectNodeRecord {
        uuid: captured.meta.uuid,
        node_type: captured.node_type.to_string(),
        user_role: captured.user_role,
        meta,
        data,
        children,
    }))
}

fn build_structural_baseline_record_for_node<T>(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
) -> Result<ProjectNodeRecord, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let recreated = recreate_node_from_capture::<T>(graph, node_id)?;
    materialize_baseline_record_for_recreated_node(graph, node_id, recreated, "Sparse Baseline Root")
}

fn build_default_baseline_record_for_node<T>(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let Some(recreated) = recreate_default_node_from_capture::<T>(graph, node_id)? else {
        return Ok(None);
    };
    materialize_baseline_record_for_recreated_node(graph, node_id, recreated, "Sparse Default Baseline Root").map(Some)
}

fn materialize_baseline_record_for_recreated_node<T>(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
    recreated: T,
    root_label: &str,
) -> Result<ProjectNodeRecord, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current = required_node(graph, node_id)?;
    let attach_under_parent = current.user_role == UserNodeRole::ItemRoot
        && current
            .parent
            .map(|parent_id| recreate_node_from_capture::<T>(graph, parent_id))
            .transpose()?
            .is_some_and(|parent| {
                parent.user_container_accepts_item(current.node_type.as_ref(), recreated.user_item_kind())
            });
    let mut temp = Engine::new(Folder::new(root_label).into());
    if attach_under_parent {
        let parent_id = current.parent.expect("accepted item roots must have a captured parent");
        let parent = recreate_node_from_capture::<T>(graph, parent_id)?;
        temp.add_node(parent, None);
        temp.apply_edits_without_creation_callbacks()?;
        let temp_parent = temp
            .nodes
            .get(temp.root)
            .and_then(|root| root.node_data().first_child)
            .ok_or_else(|| ProjectPersistenceError::Codec {
                node_type: current.node_type.to_string(),
                message: "baseline parent did not materialize".to_string(),
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
                if first.uuid == current.meta.uuid {
                    Some(first)
                } else {
                    children.find(|child| child.uuid == current.meta.uuid).or(Some(first))
                }
            })
            .ok_or_else(|| ProjectPersistenceError::Codec {
                node_type: current.node_type.to_string(),
                message: "nested baseline did not materialize the target node record".to_string(),
            });
    }
    baseline
        .root
        .children
        .into_iter()
        .next()
        .ok_or_else(|| ProjectPersistenceError::Codec {
            node_type: current.node_type.to_string(),
            message: "baseline did not materialize a node record".to_string(),
        })
}

fn recreate_node_from_capture<T>(graph: &ProjectGraphCapture, node_id: NodeId) -> Result<T, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current = required_node(graph, node_id)?;
    let mut recreated = if current.user_role == UserNodeRole::ItemRoot {
        if let Some(parent_id) = current.parent {
            let parent = recreate_node_from_capture::<T>(graph, parent_id)?;
            if let Some(mut node) = parent.create_user_item(current.node_type.as_ref()) {
                node.node_data_mut().meta.label = current.meta.label.clone();
                node.project_decode_data(&current.data)
                    .map_err(|message| codec_error(current, message))?;
                T::from_boxed_node(node).ok_or_else(|| {
                    codec_error(
                        current,
                        "parent item factory returned a node outside the engine node enum".to_string(),
                    )
                })?
            } else {
                T::project_decode_node(current.node_type.as_ref(), &current.data, &current.meta)
                    .map_err(|message| codec_error(current, message))?
            }
        } else {
            T::project_decode_node(current.node_type.as_ref(), &current.data, &current.meta)
                .map_err(|message| codec_error(current, message))?
        }
    } else {
        T::project_decode_node(current.node_type.as_ref(), &current.data, &current.meta)
            .map_err(|message| codec_error(current, message))?
    };
    let node_data = recreated.node_data_mut();
    node_data.parent = None;
    node_data.first_child = None;
    node_data.last_child = None;
    node_data.prev_sibling = None;
    node_data.next_sibling = None;
    node_data.user_role = current.user_role;
    node_data.meta = current.meta.clone();
    Ok(recreated)
}

fn recreate_default_node_from_capture<T>(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
) -> Result<Option<T>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let current = required_node(graph, node_id)?;
    if current.user_role == UserNodeRole::ItemRoot
        && let Some(parent_id) = current.parent
    {
        let parent = recreate_node_from_capture::<T>(graph, parent_id)?;
        if let Some(node) = parent.create_user_item(current.node_type.as_ref()) {
            return T::from_boxed_node(node)
                .ok_or_else(|| {
                    codec_error(
                        current,
                        "parent item factory returned a node outside the engine node enum".to_string(),
                    )
                })
                .map(Some);
        }
    }
    Ok(T::project_create_node(current.node_type.as_ref()))
}

fn project_data_for_sparse_record_capture<T: Node>(
    node: &T,
    matched_parent_baseline: Option<&ProjectNodeRecord>,
    self_baseline: Option<&ProjectNodeRecord>,
    use_declared_overlay_delta: bool,
) -> Result<Option<serde_json::Value>, ProjectPersistenceError> {
    if use_declared_overlay_delta {
        return project_persisted_data_from_runtime_capture(node, matched_parent_baseline);
    }
    if matched_parent_baseline.is_some() {
        return raw_project_data_from_runtime(node);
    }
    project_persisted_data_from_runtime_capture(node, self_baseline)
}

fn project_persisted_data_from_runtime_capture<T: Node>(
    node: &T,
    baseline: Option<&ProjectNodeRecord>,
) -> Result<Option<serde_json::Value>, ProjectPersistenceError> {
    let node_type = node.get_type().to_string();
    let data = if let Some(parameter) = node.as_any().downcast_ref::<Parameter>() {
        let persist_runtime_value = baseline.is_none() || !parameter.read_only || parameter.persist_read_only_value;
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
    .map_err(|message| ProjectPersistenceError::Codec { node_type, message })?;
    Ok((!data.is_null()).then_some(data))
}

fn sparse_encode_baseline_cache_key(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
    include_node_data: bool,
) -> Result<String, ProjectPersistenceError> {
    let node = required_node(graph, node_id)?;
    let parent = node
        .parent
        .map(|parent| sparse_node_cache_fragment(graph, parent, true))
        .transpose()?
        .unwrap_or_else(|| "root".to_string());
    let node_fragment = sparse_node_cache_fragment(graph, node_id, include_node_data)?;
    if include_node_data {
        let meta = serde_json::to_string(&project_meta_from_runtime(&node.meta))?;
        return Ok(format!("parent:{parent}\nnode:{node_fragment}\nmeta:{meta}"));
    }
    Ok(format!("parent:{parent}\nnode:{node_fragment}"))
}

fn sparse_node_cache_fragment(
    graph: &ProjectGraphCapture,
    node_id: NodeId,
    include_data: bool,
) -> Result<String, ProjectPersistenceError> {
    let node = required_node(graph, node_id)?;
    let data = include_data
        .then_some(node.data.as_ref())
        .filter(|value| !value.is_null());
    Ok(format!(
        "{}|{:?}|{}",
        node.node_type,
        node.user_role,
        sparse_json_cache_key(data)?
    ))
}

fn find_matching_child_baseline<'a>(
    child: &CapturedProjectNode,
    baseline_children: &'a [ProjectNodeRecord],
) -> Option<&'a ProjectNodeRecord> {
    baseline_children.iter().find(|baseline| {
        baseline.node_type == child.node_type.as_ref()
            && baseline.user_role == child.user_role
            && baseline.meta.decl_id.as_ref() == Some(&child.meta.decl_id)
    })
}

fn required_node(graph: &ProjectGraphCapture, node: NodeId) -> Result<&CapturedProjectNode, ProjectPersistenceError> {
    graph.get(&node).ok_or(ProjectPersistenceError::MissingNode(node))
}

fn codec_error(node: &CapturedProjectNode, message: String) -> ProjectPersistenceError {
    ProjectPersistenceError::Codec {
        node_type: node.node_type.to_string(),
        message,
    }
}

fn shard_index(node: NodeId) -> usize {
    let folded = node.0 ^ (node.0 >> 32);
    folded as usize & (PROJECT_CAPTURE_SHARD_COUNT - 1)
}
