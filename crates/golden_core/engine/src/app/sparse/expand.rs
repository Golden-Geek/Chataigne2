//! Sparse project expansion against app-owned declarations and item factories.

use std::collections::HashMap;

use crate::engine::{Engine, ProjectFile, ProjectNodeRecord, ProjectPersistenceError};
use crate::node::{Folder, NodeMeta, UserNodeRole};

use super::super::ProjectNode;
use super::{merge_sparse_record_with_baseline, sparse_json_cache_key, sparse_overlay_has_payload};

pub(in crate::app) fn expand_sparse_project_file<T>(
    project: ProjectFile,
) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let mut cache = SparseExpansionCache::default();
    Ok(ProjectFile {
        version: project.version,
        ui_state: project.ui_state,
        root: expand_sparse_node_record::<T>(&project.root, None, None, None, &mut cache)?,
    })
}

#[derive(Default)]
struct SparseExpansionCache {
    structural_baselines: HashMap<String, ProjectNodeRecord>,
}

impl SparseExpansionCache {
    fn structural_baseline_from_expanded_record<T>(
        &mut self,
        record: &ProjectNodeRecord,
        parent_node: Option<&T>,
        parent_record: Option<&ProjectNodeRecord>,
    ) -> Result<ProjectNodeRecord, ProjectPersistenceError>
    where
        T: ProjectNode + From<Folder>,
    {
        let key = sparse_expansion_baseline_cache_key(record, parent_record)?;
        if let Some(baseline) = self.structural_baselines.get(&key) {
            let mut baseline = baseline.clone();
            // Every materialization must own fresh uuids: overlay-matched children take
            // the overlay uuid during merge, but unmatched declared children keep the
            // baseline uuid and would otherwise collide across cache hits.
            crate::engine::remap_project_record_uuids(&mut baseline, &mut HashMap::new());
            // The root record's uuid is replaced by the overlay uuid during merge; keep
            // it aligned anyway for the callers that read the baseline directly.
            baseline.uuid = record.uuid;
            return Ok(baseline);
        }

        let baseline = build_structural_baseline_record_from_expanded_record::<T>(record, parent_node, parent_record)?;
        self.structural_baselines.insert(key, baseline.clone());
        Ok(baseline)
    }
}

/// Cache key for one sparse-expansion structural baseline.
///
/// The baseline output is fully determined (modulo generated uuids) by the record's
/// type, role, data and meta plus the parent context that item factories can read.
fn sparse_expansion_baseline_cache_key(
    record: &ProjectNodeRecord,
    parent_record: Option<&ProjectNodeRecord>,
) -> Result<String, ProjectPersistenceError> {
    let parent = match parent_record {
        Some(parent) => format!(
            "{}|{:?}|{}",
            parent.node_type,
            parent.user_role,
            sparse_json_cache_key(parent.data.as_ref())?
        ),
        None => "root".to_string(),
    };
    let meta = serde_json::to_string(&record.meta)?;
    Ok(format!(
        "parent:{parent}\nnode:{}|{:?}|{}\nmeta:{meta}",
        record.node_type,
        record.user_role,
        sparse_json_cache_key(record.data.as_ref())?
    ))
}

fn expand_sparse_node_record<T>(
    record: &ProjectNodeRecord,
    parent_node: Option<&T>,
    parent_record: Option<&ProjectNodeRecord>,
    matched_baseline: Option<&ProjectNodeRecord>,
    cache: &mut SparseExpansionCache,
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
                cache.structural_baseline_from_expanded_record::<T>(record, parent_node, parent_record)?;
            (
                merge_sparse_record_with_baseline(&baseline_storage, record),
                baseline_storage.children.as_slice(),
            )
        }
    };
    let current_node = decode_sparse_baseline_node(parent_node, &expanded)?;

    let children = current_node
        .project_migrate_children(&record.children)
        .map_err(|message| ProjectPersistenceError::Codec {
            node_type: record.node_type.clone(),
            message,
        })?;
    expanded.children = expand_sparse_child_records(&children, &current_node, &expanded, baseline_children, cache)?;

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

fn expand_sparse_child_records<T>(
    records: &[ProjectNodeRecord],
    current_node: &T,
    expanded_parent_record: &ProjectNodeRecord,
    baseline_children: &[ProjectNodeRecord],
    cache: &mut SparseExpansionCache,
) -> Result<Vec<ProjectNodeRecord>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let mut consumed_overlay_children = vec![false; records.len()];
    let mut expanded_children = Vec::with_capacity(baseline_children.len().max(records.len()));

    // Saved sparse children are overlays. Declared baseline children own the structural order.
    for baseline_child in baseline_children {
        let matching_indices = records
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                (!consumed_overlay_children[index] && sparse_child_matches_baseline(child, baseline_child))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let matched_index = matching_indices
            .iter()
            .copied()
            .find(|index| sparse_overlay_has_payload(&records[*index]))
            .or_else(|| matching_indices.first().copied());

        if let Some(index) = matched_index {
            if sparse_baseline_child_identity_count(baseline_children, baseline_child) > 1 {
                consumed_overlay_children[index] = true;
            } else {
                for duplicate_index in matching_indices {
                    consumed_overlay_children[duplicate_index] = true;
                }
            }
            match expand_sparse_node_record(
                &records[index],
                Some(current_node),
                Some(expanded_parent_record),
                Some(baseline_child),
                cache,
            ) {
                Ok(expanded) => expanded_children.push(expanded),
                Err(err) => {
                    // A saved overlay could not be restored onto its declared baseline
                    // (e.g. stale/renamed node type or corrupt data). Keep the declared
                    // default so the rest of the project still loads, and warn the user.
                    let overlay_child = &records[index];
                    crate::logwarning!(format!(
                        "Project load: could not restore saved node '{}' (uuid {}): {}. Falling back to declared defaults; edits under it were skipped.",
                        overlay_child.node_type, overlay_child.uuid.0, err
                    ));
                    expanded_children.push(baseline_child.clone());
                }
            }
        } else {
            expanded_children.push(baseline_child.clone());
        }
    }

    for (index, child) in records.iter().enumerate() {
        if consumed_overlay_children[index] {
            continue;
        }

        match expand_sparse_node_record(child, Some(current_node), Some(expanded_parent_record), None, cache) {
            Ok(expanded) => expanded_children.push(expanded),
            Err(err) => {
                // A saved node has no matching declared baseline and could not be decoded
                // (typically an unknown/removed node type from an older or newer project).
                // Skip it and its subtree rather than failing the whole load.
                crate::logwarning!(format!(
                    "Project load: skipping unknown or unreadable node '{}' (uuid {}) and its children: {}. The rest of the project was loaded.",
                    child.node_type, child.uuid.0, err
                ));
            }
        }
    }

    Ok(expanded_children)
}

pub(in crate::app) fn sparse_baseline_child_identity_count(
    baseline_children: &[ProjectNodeRecord],
    baseline_child: &ProjectNodeRecord,
) -> usize {
    baseline_children
        .iter()
        .filter(|candidate| sparse_child_matches_baseline(candidate, baseline_child))
        .count()
}

fn sparse_child_matches_baseline(child_record: &ProjectNodeRecord, baseline_child: &ProjectNodeRecord) -> bool {
    let Some(child_decl_id) = child_record.meta.decl_id.as_ref() else {
        return false;
    };

    baseline_child.node_type == child_record.node_type
        && baseline_child.user_role == child_record.user_role
        && baseline_child.meta.decl_id.as_ref() == Some(child_decl_id)
}
