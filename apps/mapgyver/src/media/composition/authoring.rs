use super::{InstanceNode, dependencies, protocol::*};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::{ChannelReference, ChannelSource, MediaEditResult},
};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, NodeTree, UserItemTreeInsertion},
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
    match node.definition().source {
        ChannelSource::Composition { canvas } => Ok((id, canvas)),
        _ => Err("channel is not a Composition output".into()),
    }
}

pub(in crate::media) fn instances(engine: &AppEngine, parent: NodeId) -> Result<Vec<NodeId>, String> {
    let mut next = engine
        .nodes
        .get(parent)
        .ok_or("missing Composition")?
        .node_data()
        .first_child;
    let mut result = Vec::new();
    while let Some(id) = next {
        let child = engine.nodes.get(id).ok_or("missing Composition child")?;
        next = child.node_data().next_sibling;
        if crate::parameters::is_parameter_folder(child) {
            continue;
        }
        let Some(AppNode::CompositionInstance(_)) = engine.nodes.get(id) else {
            return Err("Composition children must be instances".into());
        };
        if result.len() == MAX_INSTANCES {
            return Err("Composition exceeds 100000 authored instances".into());
        }
        result.push(id);
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

/// Reads committed instance state without settling queued operations or changing history.
pub fn inspect(engine: &AppEngine, reference: ChannelReference) -> Result<CompositionSnapshot, String> {
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
        CompositionEdit::DropSource {
            source,
            instance: target,
            position,
        } => {
            if !position.x.is_finite() || !position.y.is_finite() {
                return Err("Composition drop position must be finite canvas coordinates".into());
            }
            let source = super::super::resolve_source_node(engine, source)?;
            dependencies::reject_cycle(engine, reference, &[source])?;
            if let Some(target) = target {
                let id = instance(engine, parent, target)?;
                let AppNode::CompositionInstance(current) = engine.nodes.get(id).unwrap() else {
                    unreachable!()
                };
                let mut definition = current.definition();
                definition.source = Some(source);
                let snapshot = InstanceSnapshot {
                    reference: InstanceReference {
                        composition: reference,
                        instance: target,
                    },
                    label: current.node_data().meta.label.clone(),
                    definition,
                };
                // Preserve exact authored values, including f64 Parameter precision that
                // the render-definition projection deliberately narrows to f32.
                engine.edits.push(Edit::SetParamsAtomic {
                    values: current.source_parameter_values(source),
                    behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
                });
                Ok(MediaEditResult::InstanceChanged { snapshot })
            } else {
                let mut definition = InstanceDefinition::for_canvas(canvas);
                definition.source = Some(source);
                definition.transform.position = [position.x, position.y];
                definition.validate()?;
                append(engine, reference, parent, vec![definition])
            }
        }
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
            let previous = engine.nodes.get(parent).unwrap().node_data().last_child;
            let result = engine
                .import_project_subtrees_with(
                    imports,
                    parent,
                    previous,
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
            let AppNode::CompositionInstance(current) = engine.nodes.get(id).unwrap() else {
                unreachable!()
            };
            if current.definition().source != definition.source
                && let Some(source) = definition.source
            {
                super::super::channel(engine, source)?;
                dependencies::reject_cycle(engine, reference, &[source])?;
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
            for edit in current.parameter_edits(definition) {
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
        let node = InstanceNode::from_definition(label, definition);
        created.push(InstanceReference {
            composition: reference,
            instance: node.id(),
        });
        items.push(UserItemTreeInsertion::new(parent, NodeTree::new(node)));
    }
    engine.edits.push(Edit::AddUserItemTrees { items });
    Ok(MediaEditResult::InstancesCreated { instances: created })
}
