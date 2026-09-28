//! Backend-owned transitions between editable and compressed Mapping representations.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use golden_core::{
    app::ProjectNode,
    edit::{Edit, EditOrigin},
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
};

use crate::app::{AppEngine, AppNode};

use super::{
    frozen::{
        prepare_frozen_mapping_source, FrozenMappingBlocker,
        FrozenMappingSource,
    },
    PROCESSOR_COMPRESSION_ENABLED_DECL_ID,
    PROCESSOR_COMPRESSION_ERROR_DECL_ID, PROCESSOR_FROZEN_SOURCE_DECL_ID,
    StateProcessor,
};

const COMPRESSION_EDIT_SESSION_ID: &str = "mapping-representation-transition";

#[derive(Clone, Debug)]
pub(crate) struct PreparedMappingCompression {
    processor_uuid: NodeUuid,
    source: FrozenMappingSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MappingCompressionError {
    Blocked(Vec<FrozenMappingBlocker>),
    InvalidState(String),
    Stale,
    Persistence(String),
    Transition(String),
}

impl fmt::Display for MappingCompressionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked(blockers) => {
                let detail = blockers
                    .iter()
                    .map(|blocker| {
                        let node = blocker
                            .node
                            .map(|node| format!(" [{}]", node.0))
                            .unwrap_or_default();
                        format!("{}{}: {}", blocker.code, node, blocker.detail)
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "compression blocked: {detail}")
            }
            Self::InvalidState(detail) => formatter.write_str(detail),
            Self::Stale => formatter.write_str(
                "Mapping changed after compression preparation; prepare the transition again",
            ),
            Self::Persistence(detail) => {
                write!(formatter, "Mapping archive is invalid: {detail}")
            }
            Self::Transition(detail) => {
                write!(formatter, "Mapping representation transition failed: {detail}")
            }
        }
    }
}

pub(crate) fn prepare_mapping_compression(
    engine: &AppEngine,
    processor: NodeId,
) -> Result<PreparedMappingCompression, MappingCompressionError> {
    let processor_uuid = engine
        .nodes
        .get(processor)
        .filter(|node| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|node| node.node_data().meta.uuid)
        .ok_or_else(|| {
            MappingCompressionError::InvalidState(
                "compression target is not a live Mapping processor".to_owned(),
            )
        })?;
    if compression_enabled(engine, processor)? {
        return Err(MappingCompressionError::InvalidState(
            "Mapping is already compressed".to_owned(),
        ));
    }
    let source = prepare_frozen_mapping_source(engine, processor)
        .map_err(MappingCompressionError::Blocked)?;
    Ok(PreparedMappingCompression {
        processor_uuid,
        source,
    })
}

pub(crate) fn commit_mapping_compression(
    engine: &mut AppEngine,
    prepared: PreparedMappingCompression,
) -> Result<(), MappingCompressionError> {
    let processor = engine
        .node_id_by_uuid(prepared.processor_uuid)
        .ok_or_else(|| {
            MappingCompressionError::InvalidState(
                "Mapping disappeared before compression commit".to_owned(),
            )
        })?;
    if compression_enabled(engine, processor)? {
        return Err(MappingCompressionError::InvalidState(
            "Mapping is already compressed".to_owned(),
        ));
    }
    let current = prepare_frozen_mapping_source(engine, processor)
        .map_err(MappingCompressionError::Blocked)?;
    if current != prepared.source {
        return Err(MappingCompressionError::Stale);
    }

    let encoded = serde_json::to_string(&prepared.source)
        .map_err(|error| MappingCompressionError::Persistence(error.to_string()))?;
    let snapshot = engine.process_tree_snapshot();
    let controls = compression_controls(&snapshot, processor)?;
    let mut regions = Vec::with_capacity(prepared.source.authored_regions.len());
    for region in &prepared.source.authored_regions {
        let uuid = region.document.root.uuid;
        let node = snapshot
            .node_id_by_uuid(uuid)
            .ok_or(MappingCompressionError::Stale)?;
        if snapshot.node(node).and_then(|node| node.parent) != Some(processor) {
            return Err(MappingCompressionError::Stale);
        }
        regions.push(node);
    }
    drop(snapshot);

    let owns_session = begin_transition_session(engine, "Compress Mapping")?;
    queue_param(engine, controls.frozen_source, ParamValue::Str(encoded));
    queue_param(engine, controls.enabled, ParamValue::Bool(true));
    queue_param(
        engine,
        controls.error,
        ParamValue::Str(String::new()),
    );
    for region in regions {
        engine.edits.push(Edit::RemoveNode { node: region });
    }
    if owns_session {
        engine.edits.push(Edit::EndEditSession {
            client_edit_id: COMPRESSION_EDIT_SESSION_ID.to_owned(),
        });
    }
    engine
        .apply_edits()
        .map_err(|error| MappingCompressionError::Transition(error.to_string()))
}

pub(crate) fn set_mapping_compression(
    engine: &mut AppEngine,
    processor: NodeId,
    enabled: bool,
) -> Result<(), MappingCompressionError> {
    if enabled {
        let prepared = prepare_mapping_compression(engine, processor)?;
        commit_mapping_compression(engine, prepared)
    } else {
        expand_mapping(engine, processor)
    }
}

pub(crate) fn settle_pending_mapping_transitions(
    engine: &mut AppEngine,
) -> Result<(), String> {
    remap_colliding_frozen_archives(engine)?;
    let requests = engine
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            matches!(node, AppNode::StateProcessor(_)).then_some(node_id)
        })
        .collect::<Vec<_>>();
    for processor in requests {
        let enabled = match engine.nodes.get_mut(processor) {
            Some(AppNode::StateProcessor(processor)) => {
                processor.compression_request.take()
            }
            _ => None,
        };
        let Some(enabled) = enabled else {
            continue;
        };
        if let Err(error) = set_mapping_compression(engine, processor, enabled) {
            set_transition_error(engine, processor, &error.to_string())?;
        }
    }
    Ok(())
}

fn remap_colliding_frozen_archives(
    engine: &mut AppEngine,
) -> Result<(), String> {
    let processors = engine
        .nodes
        .iter()
        .filter_map(|(node, value)| {
            (value.get_type() == StateProcessor::NODE_TYPE).then_some(node)
        })
        .collect::<Vec<_>>();
    let snapshot = engine.process_tree_snapshot();
    let mut claimed = HashSet::new();
    let mut updates = Vec::new();
    for processor in processors {
        let Some(source) = super::processor_frozen_source(&snapshot, processor)?
        else {
            continue;
        };
        let identities = frozen_authored_uuids(&source);
        if identities.iter().all(|uuid| {
            !claimed.contains(uuid) && snapshot.node_id_by_uuid(*uuid).is_none()
        }) {
            claimed.extend(identities);
            continue;
        }
        let mut remap = HashMap::with_capacity(identities.len());
        for uuid in identities {
            let replacement = loop {
                let candidate = NodeUuid(uuid::Uuid::new_v4());
                if snapshot.node_id_by_uuid(candidate).is_none()
                    && !claimed.contains(&candidate)
                    && !remap.values().any(|current| *current == candidate)
                {
                    break candidate;
                }
            };
            remap.insert(uuid, replacement);
            claimed.insert(replacement);
        }
        let remapped = remap_frozen_source(source, &remap)?;
        let encoded = serde_json::to_string(&remapped)
            .map_err(|error| error.to_string())?;
        let control = snapshot
            .find_child_by_decl_id(processor, PROCESSOR_FROZEN_SOURCE_DECL_ID)
            .ok_or_else(|| {
                "compressed Mapping duplicate has no frozen source control"
                    .to_owned()
            })?;
        updates.push((control, encoded));
    }
    drop(snapshot);
    for (control, encoded) in updates {
        queue_param(engine, control, ParamValue::Str(encoded));
    }
    if !engine.edits.pending.is_empty() {
        engine.apply_edits().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn frozen_authored_uuids(source: &FrozenMappingSource) -> Vec<NodeUuid> {
    fn collect(
        record: &golden_core::engine::ProjectNodeRecord,
        identities: &mut Vec<NodeUuid>,
    ) {
        identities.push(record.uuid);
        for child in &record.children {
            collect(child, identities);
        }
    }

    let mut identities = Vec::new();
    for region in &source.authored_regions {
        collect(&region.document.root, &mut identities);
    }
    identities
}

fn remap_frozen_source(
    source: FrozenMappingSource,
    remap: &HashMap<NodeUuid, NodeUuid>,
) -> Result<FrozenMappingSource, String> {
    fn remap_json(
        value: &mut serde_json::Value,
        remap: &HashMap<String, String>,
    ) {
        match value {
            serde_json::Value::String(value) => {
                if let Some(replacement) = remap.get(value) {
                    *value = replacement.clone();
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    remap_json(value, remap);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values_mut() {
                    remap_json(value, remap);
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_) => {}
        }
    }

    let remap = remap
        .iter()
        .map(|(from, to)| (from.0.to_string(), to.0.to_string()))
        .collect::<HashMap<_, _>>();
    let mut value = serde_json::to_value(source).map_err(|error| error.to_string())?;
    remap_json(&mut value, &remap);
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn expand_mapping(
    engine: &mut AppEngine,
    processor: NodeId,
) -> Result<(), MappingCompressionError> {
    if !compression_enabled(engine, processor)? {
        return Err(MappingCompressionError::InvalidState(
            "Mapping is already editable".to_owned(),
        ));
    }
    let snapshot = engine.process_tree_snapshot();
    let controls = compression_controls(&snapshot, processor)?;
    let encoded = snapshot
        .node(controls.frozen_source)
        .and_then(|node| node.param_value.as_ref())
        .and_then(|value| match value {
            ParamValue::Str(value) => Some(value.clone()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            MappingCompressionError::InvalidState(
                "compressed Mapping has no frozen source".to_owned(),
            )
        })?;
    let source: FrozenMappingSource = serde_json::from_str(&encoded)
        .map_err(|error| MappingCompressionError::Persistence(error.to_string()))?;
    source
        .compile_runtime()
        .map_err(MappingCompressionError::Persistence)?;
    let archived_uuids = frozen_authored_uuids(&source);
    let mut unique_uuids = HashSet::with_capacity(archived_uuids.len());
    for uuid in archived_uuids {
        if !unique_uuids.insert(uuid) {
            return Err(MappingCompressionError::Persistence(format!(
                "archived workflow UUID {} appears more than once",
                uuid.0
            )));
        }
        if snapshot.node_id_by_uuid(uuid).is_some() {
            return Err(MappingCompressionError::InvalidState(format!(
                "archived workflow UUID {} is already live",
                uuid.0
            )));
        }
    }
    let previous = snapshot.child_ids(processor).last().copied();
    drop(snapshot);

    let owns_session = begin_transition_session(engine, "Expand Mapping")?;
    let documents = source
        .authored_regions
        .into_iter()
        .map(|region| region.document)
        .collect();
    if let Err(error) = engine.restore_project_subtrees_with(
        documents,
        processor,
        previous,
        AppNode::project_decode_node,
    ) {
        close_failed_transition_session(engine, owns_session);
        return Err(MappingCompressionError::Persistence(error.to_string()));
    }
    queue_param(engine, controls.enabled, ParamValue::Bool(false));
    queue_param(
        engine,
        controls.frozen_source,
        ParamValue::Str(String::new()),
    );
    queue_param(
        engine,
        controls.error,
        ParamValue::Str(String::new()),
    );
    if owns_session {
        engine.edits.push(Edit::EndEditSession {
            client_edit_id: COMPRESSION_EDIT_SESSION_ID.to_owned(),
        });
    }
    engine
        .apply_edits()
        .map_err(|error| MappingCompressionError::Transition(error.to_string()))
}

#[derive(Clone, Copy)]
struct CompressionControls {
    enabled: NodeId,
    error: NodeId,
    frozen_source: NodeId,
}

fn compression_controls(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    processor: NodeId,
) -> Result<CompressionControls, MappingCompressionError> {
    let find = |decl_id: &str| {
        snapshot
            .find_child_by_decl_id(processor, decl_id)
            .ok_or_else(|| {
                MappingCompressionError::InvalidState(format!(
                    "Mapping compression control '{decl_id}' is missing"
                ))
            })
    };
    Ok(CompressionControls {
        enabled: find(PROCESSOR_COMPRESSION_ENABLED_DECL_ID)?,
        error: find(PROCESSOR_COMPRESSION_ERROR_DECL_ID)?,
        frozen_source: find(PROCESSOR_FROZEN_SOURCE_DECL_ID)?,
    })
}

fn compression_enabled(
    engine: &AppEngine,
    processor: NodeId,
) -> Result<bool, MappingCompressionError> {
    let snapshot = engine.process_tree_snapshot();
    let enabled = compression_controls(&snapshot, processor)?.enabled;
    Ok(snapshot
        .node(enabled)
        .and_then(|node| node.param_value.as_ref())
        .is_some_and(|value| matches!(value, ParamValue::Bool(true))))
}

fn begin_transition_session(
    engine: &mut AppEngine,
    label: &str,
) -> Result<bool, MappingCompressionError> {
    if engine.has_active_edit_session() {
        return Ok(false);
    }
    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Runtime,
        label: Some(label.to_owned()),
        client_edit_id: COMPRESSION_EDIT_SESSION_ID.to_owned(),
        ui_client_instance_id: None,
    });
    engine
        .apply_edits()
        .map_err(|error| MappingCompressionError::Transition(error.to_string()))?;
    Ok(true)
}

fn close_failed_transition_session(engine: &mut AppEngine, owns_session: bool) {
    if owns_session
        && engine.active_edit_session_id()
            == Some(COMPRESSION_EDIT_SESSION_ID)
    {
        engine.edits.push(Edit::EndEditSession {
            client_edit_id: COMPRESSION_EDIT_SESSION_ID.to_owned(),
        });
        let _ = engine.apply_edits();
    }
}

fn queue_param(engine: &mut AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Append,
    });
}

fn set_transition_error(
    engine: &mut AppEngine,
    processor: NodeId,
    detail: &str,
) -> Result<(), String> {
    let snapshot = engine.process_tree_snapshot();
    let controls = compression_controls(&snapshot, processor)
        .map_err(|error| error.to_string())?;
    drop(snapshot);
    queue_param(
        engine,
        controls.error,
        ParamValue::Str(detail.to_owned()),
    );
    engine.apply_edits().map_err(|error| error.to_string())
}
