use super::{children, dependencies, protocol::*};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::{ChannelReference, ChannelSource, MediaEditResult},
};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, UserItemTreeInsertion},
    engine::ProjectImportSpec,
    node::{Node, NodeId},
};
use std::collections::HashSet;

pub const MAX_INSTANCES: usize = 100_000;
pub const MAX_ADD_BATCH: usize = 4096;

pub(super) fn channel(
    engine: &AppEngine,
    reference: ChannelReference,
) -> Result<(NodeId, golden_video::Extent2D), String> {
    let id = super::super::authoring::channel_id(engine, reference)?;
    let AppNode::MediaChannel(node) = engine.nodes.get(id).unwrap() else {
        unreachable!()
    };
    match super::super::authoring::effective_definition(engine, node.as_ref())?.source {
        ChannelSource::Composition { canvas } => Ok((id, canvas)),
        _ => Err("channel is not a Composition output".into()),
    }
}

pub(in crate::media) fn instances(engine: &AppEngine, parent: NodeId) -> Result<Vec<NodeId>, String> {
    instance_nodes(engine, parent)?.collect()
}

/// Walk authored membership without allocating the entire Composition for paged readers.
pub(super) fn instance_nodes(
    engine: &AppEngine,
    parent: NodeId,
) -> Result<impl Iterator<Item = Result<NodeId, String>> + '_, String> {
    let parent = children::owner(engine, parent)?;
    let mut next = engine
        .nodes
        .get(parent)
        .ok_or("missing Composition")?
        .node_data()
        .first_child;
    let mut count = 0;
    Ok(std::iter::from_fn(move || {
        while let Some(id) = next {
            let Some(child) = engine.nodes.get(id) else {
                next = None;
                return Some(Err("missing Composition child".into()));
            };
            next = child.node_data().next_sibling;
            if crate::parameters::is_parameter_folder(child)
                || matches!(child, AppNode::MediaChannel(_) | AppNode::CompositionPlacement(_))
            {
                continue;
            }
            if !matches!(child, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
                next = None;
                return Some(Err("Composition children must be media".into()));
            }
            if count == MAX_INSTANCES {
                next = None;
                return Some(Err("Composition exceeds 100000 authored instances".into()));
            }
            count += 1;
            return Some(Ok(id));
        }
        None
    }))
}

pub(super) fn instance(engine: &AppEngine, parent: NodeId, instance: InstanceId) -> Result<NodeId, String> {
    let parent = children::owner(engine, parent)?;
    let id = engine.node_id_by_uuid(instance.0).ok_or("instance is missing")?;
    match engine.nodes.get(id) {
        Some(node @ (AppNode::Media(_) | AppNode::ReferenceMedia(_))) if node.node_data().parent == Some(parent) => {
            Ok(id)
        }
        _ => Err("instance belongs to another Composition".into()),
    }
}

/// Reads committed instance state without settling queued operations or changing history.
pub fn inspect(engine: &AppEngine, reference: ChannelReference) -> Result<CompositionSnapshot, String> {
    let (parent, canvas) = channel(engine, reference)?;
    let instances = instances(engine, parent)?
        .into_iter()
        .map(|id| {
            let node = engine.nodes.get(id).unwrap();
            Ok(InstanceSnapshot {
                reference: InstanceReference {
                    composition: reference,
                    instance: InstanceId(node.node_data().meta.uuid),
                },
                label: node.node_data().meta.label.clone(),
                definition: children::definition(engine, id)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CompositionSnapshot {
        reference,
        canvas,
        instances,
    })
}

pub(crate) fn apply(
    engine: &mut AppEngine,
    reference: ChannelReference,
    edit: CompositionEdit,
) -> Result<MediaEditResult, String> {
    let (parent, canvas) = channel(engine, reference)?;
    match edit {
        CompositionEdit::CreateMedia {
            media_kind,
            label,
            position,
        } => super::creation::apply(engine, reference, parent, canvas, media_kind, label, position),
        CompositionEdit::Inspect => Ok(MediaEditResult::Composition {
            snapshot: inspect(engine, reference)?,
        }),
        CompositionEdit::AddInstances { sources } => {
            if sources.is_empty() || sources.len() > MAX_ADD_BATCH {
                return Err("add 1..4096 instances per atomic batch".into());
            }
            for source in &sources {
                super::super::channel(engine, *source)?;
            }
            dependencies::reject_cycle(engine, reference, &sources)?;
            append(
                engine,
                reference,
                parent,
                sources
                    .into_iter()
                    .map(|source| {
                        (
                            crate::media::MediaSource {
                                media: source.media,
                                channel: Some(source.channel),
                            },
                            InstanceDefinition {
                                source: Some(source),
                                ..InstanceDefinition::for_canvas(canvas)
                            },
                        )
                    })
                    .collect(),
            )
        }
        CompositionEdit::DropSource {
            source,
            instance: target,
            position,
            copy,
        } => super::drops::apply(
            engine,
            reference,
            parent,
            canvas,
            super::drops::DropRequest {
                source,
                target,
                position,
                copy,
            },
        ),
        CompositionEdit::DuplicateInstances { instances: targets } => {
            if targets.is_empty() || targets.len() > MAX_ADD_BATCH {
                return Err("duplicate 1..4096 instances per atomic batch".into());
            }
            if instances(engine, parent)?.len().saturating_add(targets.len()) > MAX_INSTANCES {
                return Err("Composition exceeds 100000 authored instances".into());
            }
            let mut unique = HashSet::new();
            let imports = targets
                .into_iter()
                .map(|target| {
                    let id = instance(engine, parent, target)?;
                    if !unique.insert(id) {
                        return Err("duplicate targets must be distinct".into());
                    }
                    // Capture the full authored subtree, including constraints/defaults and
                    // control nodes. A value-only definition loses normal Parameter state.
                    let project = engine
                        .capture_project_subtree_with(id, Node::project_encode_data)
                        .map_err(|error| error.to_string())?;
                    Ok(ProjectImportSpec {
                        project,
                        initial_params: Vec::new(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let result = engine
                .import_project_subtrees_with(
                    imports,
                    children::owner(engine, parent)?,
                    engine
                        .nodes
                        .get(children::owner(engine, parent)?)
                        .unwrap()
                        .node_data()
                        .last_child,
                    AppNode::project_decode_node,
                    // These are internally captured, already admitted instance subtrees.
                    |_, _| Ok(()),
                )
                .map_err(|error| error.to_string())?;
            Ok(MediaEditResult::InstancesCreated {
                instances: result
                    .roots
                    .into_iter()
                    .map(|id| InstanceReference {
                        composition: reference,
                        instance: InstanceId(engine.nodes.get(id).unwrap().node_data().meta.uuid),
                    })
                    .collect(),
            })
        }
        CompositionEdit::ConfigureInstance {
            instance: target,
            definition,
        } => {
            let id = instance(engine, parent, target)?;
            definition.validate()?;
            let current = children::placement(engine, id)?;
            if children::definition(engine, id)?.source != definition.source {
                return Err("Edit child media Target/Output to change its source".into());
            }
            for edit in current.parameter_edits(definition) {
                engine.edits.push(edit);
            }
            Ok(MediaEditResult::Changed)
        }
        CompositionEdit::RenameInstance {
            instance: target,
            label,
        } => {
            let id = instance(engine, parent, target)?;
            labels::rename(engine, id, label)?;
            Ok(MediaEditResult::Changed)
        }
        CompositionEdit::EditInstanceGeometry { instance: target, edit } => {
            let id = instance(engine, parent, target)?;
            let current = children::placement(engine, id)?;
            let mut definition = children::definition(engine, id)?;
            if let InstanceGeometryMode::Resize { extent, .. } = edit.mode {
                let source = definition
                    .source
                    .ok_or("source dimensions are unavailable for resize")?;
                if crate::media::resolution::channel_extent(engine, source)? != extent {
                    return Err("source dimensions changed; start a new resize gesture".into());
                }
            }
            edit.apply(&mut definition)?;
            let snapshot = InstanceSnapshot {
                reference: InstanceReference {
                    composition: reference,
                    instance: target,
                },
                label: engine.nodes.get(id).unwrap().node_data().meta.label.clone(),
                definition: definition.clone(),
            };
            for edit in current.geometry_edits(definition, edit.mode) {
                engine.edits.push(edit);
            }
            Ok(MediaEditResult::InstanceChanged { snapshot })
        }
        CompositionEdit::RemoveInstance { instance: target } => {
            let id = instance(engine, parent, target)?;
            engine.edits.push(Edit::RemoveNode { node: id });
            Ok(MediaEditResult::Changed)
        }
        CompositionEdit::ReorderInstance {
            instance: target,
            after,
        } => {
            let id = instance(engine, parent, target)?;
            let previous = after.map(|target| instance(engine, parent, target)).transpose()?;
            if previous == Some(id) {
                return Err("instance cannot follow itself".into());
            }
            engine.edits.push(Edit::MoveNode {
                node: id,
                new_parent: children::owner(engine, parent)?,
                new_prev_sibling: previous,
            });
            Ok(MediaEditResult::Changed)
        }
    }
}

pub(super) fn append(
    engine: &mut AppEngine,
    reference: ChannelReference,
    parent: NodeId,
    definitions: Vec<(crate::media::MediaSource, InstanceDefinition)>,
) -> Result<MediaEditResult, String> {
    let existing = instances(engine, parent)?;
    if existing.len().saturating_add(definitions.len()) > MAX_INSTANCES {
        return Err("Composition exceeds 100000 authored instances".into());
    }
    let mut labels = existing
        .into_iter()
        .map(|id| engine.nodes.get(id).unwrap().node_data().meta.label.clone())
        .collect::<HashSet<_>>();
    let mut next_label = 1u64;
    let mut created = Vec::with_capacity(definitions.len());
    let mut items = Vec::with_capacity(definitions.len());
    for (source, definition) in definitions {
        let label = loop {
            let candidate = format!("Instance {next_label}");
            next_label += 1;
            if labels.insert(candidate.clone()) {
                break candidate;
            }
        };
        let (tree, id) = children::reference_tree(label, Some(source), definition);
        created.push(InstanceReference {
            composition: reference,
            instance: id,
        });
        items.push(UserItemTreeInsertion::new(children::owner(engine, parent)?, tree));
    }
    engine.edits.push(Edit::AddUserItemTrees { items });
    Ok(MediaEditResult::InstancesCreated { instances: created })
}
