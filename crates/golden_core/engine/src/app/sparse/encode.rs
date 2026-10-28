//! Sparse encoding and default/structural baseline materialization.

use std::collections::{HashMap, HashSet};

use crate::engine::{Engine, ProjectNodeRecord, ProjectPersistenceError};
use crate::node::{Folder, Node, NodeId, NodeUuid, UserNodeRole};

use super::super::{ProjectNode, node_is_app_data_persisted};
use super::{
    project_data_for_sparse_record, project_meta_delta_from_runtime, project_meta_for_sparse_record,
    project_meta_from_runtime, project_persisted_data_from_runtime, raw_project_data_from_runtime,
    sparse_baseline_child_identity_count, sparse_json_cache_key,
};

#[derive(Default)]
pub(in crate::app) struct SparseEncodeCache {
    structural_baselines: HashMap<String, ProjectNodeRecord>,
    default_baselines: HashMap<String, Option<ProjectNodeRecord>>,
}

impl SparseEncodeCache {
    pub(in crate::app) fn structural_baseline_for_node<T>(
        &mut self,
        engine: &Engine<T>,
        node_id: NodeId,
    ) -> Result<ProjectNodeRecord, ProjectPersistenceError>
    where
        T: ProjectNode + From<Folder>,
    {
        let key = sparse_encode_baseline_cache_key(engine, node_id, true)?;
        if let Some(baseline) = self.structural_baselines.get(&key) {
            let mut baseline = baseline.clone();
            let node = engine
                .nodes
                .get(node_id)
                .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
            baseline.uuid = node.node_data().meta.uuid;
            baseline.meta = project_meta_from_runtime(&node.node_data().meta);
            return Ok(baseline);
        }

        let baseline = build_structural_baseline_record_for_node(engine, node_id)?;
        self.structural_baselines.insert(key, baseline.clone());
        Ok(baseline)
    }

    fn default_baseline_for_node<T>(
        &mut self,
        engine: &Engine<T>,
        node_id: NodeId,
    ) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
    where
        T: ProjectNode + From<Folder>,
    {
        let key = sparse_encode_baseline_cache_key(engine, node_id, false)?;
        if let Some(baseline) = self.default_baselines.get(&key) {
            return Ok(baseline.clone());
        }

        let baseline = build_default_baseline_record_for_node(engine, node_id)?;
        self.default_baselines.insert(key, baseline.clone());
        Ok(baseline)
    }
}

fn sparse_encode_baseline_cache_key<T>(
    engine: &Engine<T>,
    node_id: NodeId,
    include_node_data: bool,
) -> Result<String, ProjectPersistenceError>
where
    T: Node,
{
    let node = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
    let parent = node
        .node_data()
        .parent
        .map(|parent_id| sparse_runtime_node_cache_fragment(engine, parent_id, true))
        .transpose()?
        .unwrap_or_else(|| "root".to_string());
    let node_fragment = sparse_runtime_node_cache_fragment(engine, node_id, include_node_data)?;
    if include_node_data {
        let node = engine
            .nodes
            .get(node_id)
            .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
        let meta = serde_json::to_string(&project_meta_from_runtime(&node.node_data().meta))?;
        return Ok(format!("parent:{parent}\nnode:{node_fragment}\nmeta:{meta}"));
    }
    Ok(format!("parent:{parent}\nnode:{node_fragment}"))
}

fn sparse_runtime_node_cache_fragment<T>(
    engine: &Engine<T>,
    node_id: NodeId,
    include_data: bool,
) -> Result<String, ProjectPersistenceError>
where
    T: Node,
{
    let node = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;
    let data = if include_data {
        raw_project_data_from_runtime(node)?
    } else {
        None
    };
    Ok(format!(
        "{}|{:?}|{}",
        node.get_type(),
        node.node_data().user_role,
        sparse_json_cache_key(data.as_ref())?
    ))
}

pub(in crate::app) fn encode_sparse_node_record<T>(
    engine: &Engine<T>,
    node_id: NodeId,
    matched_parent_baseline: Option<&ProjectNodeRecord>,
    allow_omission: bool,
    omit_app_data_nodes: bool,
    referenced_uuids: &HashSet<NodeUuid>,
    cache: &mut SparseEncodeCache,
) -> Result<Option<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let node = engine
        .nodes
        .get(node_id)
        .ok_or(ProjectPersistenceError::MissingNode(node_id))?;

    if omit_app_data_nodes && node_is_app_data_persisted(node) {
        return Ok(None);
    }

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
        structural_baseline_storage = Some(cache.structural_baseline_for_node(engine, node_id)?);
        structural_baseline_storage
            .as_ref()
            .map(|baseline| baseline.children.as_slice())
            .unwrap_or(&[])
    };

    let default_baseline_storage;
    let self_baseline = if let Some(baseline) = matched_parent_baseline {
        Some(baseline)
    } else {
        default_baseline_storage = cache.default_baseline_for_node(engine, node_id)?;
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
        let repeated_child_baseline = matched_child_baseline
            .is_some_and(|baseline| sparse_baseline_child_identity_count(baseline_children, baseline) > 1);
        let child_allow_omission = matched_child_baseline.is_some() && !repeated_child_baseline;
        if let Some(record) = encode_sparse_node_record(
            engine,
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

    if current.node_data().user_role == UserNodeRole::ItemRoot
        && let Some(parent_id) = current.node_data().parent
    {
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
