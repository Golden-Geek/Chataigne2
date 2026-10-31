use super::{InstanceNode, dependencies, protocol::*};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::{ChannelReference, ChannelSource, MediaEditResult},
};
use golden_core::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
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
    match node.definition().source {
        ChannelSource::Composition { canvas } => Ok((id, canvas)),
        _ => Err("channel is not a Composition output".into()),
    }
}

pub(super) fn instances(engine: &AppEngine, parent: NodeId) -> Result<Vec<NodeId>, String> {
    let mut next = engine
        .nodes
        .get(parent)
        .ok_or("missing Composition")?
        .node_data()
        .first_child;
    let mut result = Vec::new();
    while let Some(id) = next {
        let Some(AppNode::CompositionInstance(node)) = engine.nodes.get(id) else {
            return Err("Composition children must be instances".into());
        };
        if result.len() == MAX_INSTANCES {
            return Err("Composition exceeds 100000 authored instances".into());
        }
        result.push(id);
        next = node.node_data().next_sibling;
    }
    Ok(result)
}

fn instance(engine: &AppEngine, parent: NodeId, instance: InstanceId) -> Result<NodeId, String> {
    let id = engine.node_id_by_uuid(instance.0).ok_or("instance is missing")?;
    match engine.nodes.get(id) {
        Some(AppNode::CompositionInstance(node)) if node.node_data().parent == Some(parent) => Ok(id),
        _ => Err("instance belongs to another Composition".into()),
    }
}

pub(super) fn inspect(engine: &AppEngine, reference: ChannelReference) -> Result<CompositionSnapshot, String> {
    let (parent, canvas) = channel(engine, reference)?;
    let instances = instances(engine, parent)?
        .into_iter()
        .map(|id| {
            let AppNode::CompositionInstance(node) = engine.nodes.get(id).unwrap() else {
                unreachable!()
            };
            InstanceSnapshot {
                reference: InstanceReference {
                    composition: reference,
                    instance: node.id(),
                },
                label: node.node_data().meta.label.clone(),
                definition: node.definition().clone(),
            }
        })
        .collect();
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
                    .map(|source| InstanceDefinition {
                        source: Some(source),
                        ..InstanceDefinition::for_canvas(canvas)
                    })
                    .collect(),
            )
        }
        CompositionEdit::DuplicateInstances { instances: targets } => {
            if targets.is_empty() || targets.len() > MAX_ADD_BATCH {
                return Err("duplicate 1..4096 instances per atomic batch".into());
            }
            let definitions = targets
                .into_iter()
                .map(|target| {
                    let id = instance(engine, parent, target)?;
                    let AppNode::CompositionInstance(node) = engine.nodes.get(id).unwrap() else {
                        unreachable!()
                    };
                    Ok(node.definition().clone())
                })
                .collect::<Result<Vec<_>, String>>()?;
            append(engine, reference, parent, definitions)
        }
        CompositionEdit::ConfigureInstance {
            instance: target,
            definition,
        } => {
            let id = instance(engine, parent, target)?;
            definition.validate()?;
            let AppNode::CompositionInstance(current) = engine.nodes.get(id).unwrap() else {
                unreachable!()
            };
            if current.definition().source != definition.source
                && let Some(source) = definition.source
            {
                super::super::channel(engine, source)?;
                dependencies::reject_cycle(engine, reference, &[source])?;
            }
            engine.edits.push(Edit::CallNodeMutation {
                node: id,
                needs_tree_snapshot: false,
                callback: Box::new(move |node, _| {
                    node.as_any_mut()
                        .downcast_mut::<InstanceNode>()
                        .ok_or("instance type changed")?
                        .set_definition(definition);
                    Ok(())
                }),
            });
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
            let AppNode::CompositionInstance(current) = engine.nodes.get(id).unwrap() else {
                unreachable!()
            };
            let mut definition = current.definition().clone();
            edit.apply(&mut definition)?;
            let snapshot = InstanceSnapshot {
                reference: InstanceReference {
                    composition: reference,
                    instance: target,
                },
                label: current.node_data().meta.label.clone(),
                definition: definition.clone(),
            };
            engine.edits.push(Edit::CallNodeMutationCoalesced {
                scope: golden_core::edit::NodeMutationScope::SelfData,
                node: id,
                needs_tree_snapshot: false,
                callback: Box::new(move |node, _| {
                    node.as_any_mut()
                        .downcast_mut::<InstanceNode>()
                        .ok_or("instance type changed")?
                        .set_definition(definition);
                    Ok(())
                }),
            });
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
                new_parent: parent,
                new_prev_sibling: previous,
            });
            Ok(MediaEditResult::Changed)
        }
    }
}

fn append(
    engine: &mut AppEngine,
    reference: ChannelReference,
    parent: NodeId,
    definitions: Vec<InstanceDefinition>,
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
    for definition in definitions {
        let label = loop {
            let candidate = format!("Instance {next_label}");
            next_label += 1;
            if labels.insert(candidate.clone()) {
                break candidate;
            }
        };
        let node = InstanceNode::new(label, definition);
        created.push(InstanceReference {
            composition: reference,
            instance: node.id(),
        });
        items.push(UserItemTreeInsertion::new(parent, NodeTree::new(node)));
    }
    engine.edits.push(Edit::AddUserItemTrees { items });
    Ok(MediaEditResult::InstancesCreated { instances: created })
}
