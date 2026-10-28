//! Shared sparse record, metadata and payload rules.

use std::collections::HashSet;

use crate::engine::{Engine, ProjectNodeMeta, ProjectNodeRecord, ProjectPersistenceError};
use crate::node::{Node, NodeMeta, NodeUuid};
use crate::parameter::Parameter;

mod encode;
mod expand;

pub(super) use encode::{SparseEncodeCache, encode_sparse_node_record};
pub(super) use expand::{expand_sparse_project_file, sparse_baseline_child_identity_count};

pub(super) fn sparse_json_cache_key(value: Option<&serde_json::Value>) -> Result<String, ProjectPersistenceError> {
    match value {
        Some(value) => Ok(serde_json::to_string(value)?),
        None => Ok("null".to_string()),
    }
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

fn sparse_overlay_has_payload(record: &ProjectNodeRecord) -> bool {
    record.data.is_some() || !record.children.is_empty() || sparse_meta_has_payload(&record.meta)
}

fn sparse_meta_has_payload(meta: &ProjectNodeMeta) -> bool {
    meta.short_name.is_some()
        || meta.enabled.is_some()
        || meta.can_be_disabled.is_some()
        || meta.label.is_some()
        || meta.description.is_some()
        || meta.declared_description_key.is_some()
        || meta.declared_description.is_some()
        || meta.tags.is_some()
        || meta.user_permissions.is_some()
        || meta.semantics.is_some()
        || meta.presentation.is_some()
}

pub(super) fn project_meta_from_runtime(meta: &NodeMeta) -> ProjectNodeMeta {
    ProjectNodeMeta::from_runtime(meta).without_runtime_fields()
}

pub(super) fn project_meta_for_sparse_record(
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

pub(super) fn project_meta_delta_from_runtime(meta: &NodeMeta, baseline: Option<&ProjectNodeMeta>) -> ProjectNodeMeta {
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
    .map_err(|message| ProjectPersistenceError::Codec {
        node_type: node_type.clone(),
        message,
    })?;

    Ok((!data_value.is_null()).then_some(data_value))
}

pub(super) fn raw_project_data_from_runtime<T: Node>(
    node: &T,
) -> Result<Option<serde_json::Value>, ProjectPersistenceError> {
    let node_type = node.get_type().to_string();
    let data_value = node
        .project_encode_data()
        .map_err(|message| ProjectPersistenceError::Codec {
            node_type: node_type.clone(),
            message,
        })?;

    Ok((!data_value.is_null()).then_some(data_value))
}

pub(super) fn diff_project_data_against_baseline(
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

pub(super) fn collect_referenced_uuids<T: Node>(engine: &Engine<T>) -> HashSet<NodeUuid> {
    let mut referenced_uuids = HashSet::<NodeUuid>::new();
    for (_, node) in engine.nodes.iter() {
        node.engine_visit_references(&mut |reference| {
            referenced_uuids.insert(reference.uuid());
        });
    }
    referenced_uuids
}
