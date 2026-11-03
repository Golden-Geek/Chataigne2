use super::{PlacementNode, children, dependencies, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, NodeTree},
    engine::ProjectImportSpec,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
};

pub(super) struct DropRequest {
    pub source: NodeUuid,
    pub target: Option<InstanceId>,
    pub position: Point,
    pub copy: bool,
}

pub(super) fn apply(
    engine: &mut AppEngine,
    reference: ChannelReference,
    channel: NodeId,
    canvas: golden_video::Extent2D,
    request: DropRequest,
) -> Result<MediaEditResult, String> {
    let DropRequest {
        source,
        target,
        position,
        copy,
    } = request;
    if !position.x.is_finite() || !position.y.is_finite() {
        return Err("Composition drop position must be finite canvas coordinates".into());
    }
    let source = crate::media::source_selection(engine, source)?;
    if !copy {
        dependencies::reject_cycle(
            engine,
            reference,
            &source.output(engine)?.into_iter().collect::<Vec<_>>(),
        )?;
    }
    let parent = children::owner(engine, channel)?;
    let existing = target
        .map(|target| {
            let id = engine.node_id_by_uuid(target.0).ok_or("media child is missing")?;
            if engine.nodes.get(id).unwrap().node_data().parent != Some(parent) {
                return Err("media belongs to another Composition");
            }
            Ok(id)
        })
        .transpose()
        .map_err(str::to_owned)?;
    if !copy {
        if let Some(id) = existing {
            if let Some(AppNode::ReferenceMedia(node)) = engine.nodes.get(id) {
                let target_parameter = node.target_parameter();
                let placement = children::placement(engine, id)?;
                let mut definition = children::definition(engine, id)?;
                definition.source = source.channel.map(|channel| ChannelReference {
                    media: MediaId(target.unwrap().0),
                    channel,
                });
                engine.edits.push(Edit::SetParamsAtomic {
                    values: vec![
                        (
                            target_parameter,
                            ParamValue::Reference(golden_core::node::NodeReference::new(source.media.0)),
                        ),
                        (
                            placement.output_parameter(),
                            ParamValue::Reference(
                                source
                                    .channel
                                    .map_or_else(golden_core::node::NodeReference::empty, |channel| {
                                        golden_core::node::NodeReference::new(channel.0)
                                    }),
                            ),
                        ),
                    ],
                    behaviour: ParameterEventBehaviour::Append,
                });
                return Ok(MediaEditResult::InstanceChanged {
                    snapshot: InstanceSnapshot {
                        reference: InstanceReference {
                            composition: reference,
                            instance: target.unwrap(),
                        },
                        label: engine.nodes.get(id).unwrap().node_data().meta.label.clone(),
                        definition,
                    },
                });
            }
            return replace_with_reference(engine, reference, parent, id, source);
        }
        let mut definition = InstanceDefinition::for_canvas(canvas);
        definition.source = source.output(engine)?;
        definition.transform.position = [position.x, position.y];
        return super::authoring::append(engine, reference, channel, vec![(source, definition)]);
    }
    let root = engine
        .node_id_by_uuid(source.media.0)
        .ok_or("source media is missing")?;
    let mut project = engine
        .capture_project_subtree_with(root, Node::project_encode_data)
        .map_err(|e| e.to_string())?;
    if let Some(old) = existing {
        let placement = children::placement(engine, old)?;
        let retained = engine
            .capture_project_subtree_with(placement.node_data().id, Node::project_encode_data)
            .map_err(|e| e.to_string())?;
        project
            .root
            .children
            .retain(|node| node.node_type != super::PLACEMENT_TYPE);
        project.root.children.push(retained.root);
    }
    let result = engine
        .import_project_subtrees_with(
            vec![ProjectImportSpec {
                project,
                initial_params: vec![],
            }],
            parent,
            engine.nodes.get(parent).unwrap().node_data().last_child,
            AppNode::project_decode_node,
            |_, _| Ok(()),
        )
        .map_err(|e| e.to_string())?;
    let id = result.roots[0];
    let uuid = engine.nodes.get(id).unwrap().node_data().meta.uuid;
    let output = source.channel.map(|channel| {
        result
            .identities
            .iter()
            .find(|item| item.source_uuid == channel.0)
            .map_or(channel.0, |item| item.uuid)
    });
    let selected = output.map(|output| ChannelReference {
        media: MediaId(uuid),
        channel: ChannelId(output),
    });
    if let Ok(placement) = children::placement(engine, id) {
        let mut values = vec![(
            placement.output_parameter(),
            ParamValue::Reference(output.map_or_else(
                golden_core::node::NodeReference::empty,
                golden_core::node::NodeReference::new,
            )),
        )];
        if existing.is_none() {
            values.push((
                placement.position_parameter(),
                ParamValue::Vec2(position.x.into(), position.y.into()),
            ));
        }
        engine.edits.push(Edit::SetParamsAtomic {
            values,
            behaviour: ParameterEventBehaviour::Append,
        });
    } else {
        let mut definition = InstanceDefinition::for_canvas(canvas);
        definition.source = selected;
        definition.transform.position = [position.x, position.y];
        engine.edits.push(Edit::AddNodeTree {
            parent: id,
            prev_sibling: None,
            tree: NodeTree::new(PlacementNode::from_definition("Placement".into(), definition)),
        });
    }
    if let Some(old) = existing {
        engine.edits.push(Edit::RemoveNode { node: old });
    }
    Ok(MediaEditResult::InstancesCreated {
        instances: vec![InstanceReference {
            composition: reference,
            instance: InstanceId(uuid),
        }],
    })
}

fn replace_with_reference(
    engine: &mut AppEngine,
    reference: ChannelReference,
    parent: NodeId,
    old: NodeId,
    source: crate::media::MediaSource,
) -> Result<MediaEditResult, String> {
    let label = engine.nodes.get(old).unwrap().node_data().meta.label.clone();
    let mut definition = children::definition(engine, old)?;
    definition.source = source.output(engine)?;
    let (tree, instance) = children::reference_tree("Replacement".into(), Some(source), definition.clone());
    engine.edits.push(Edit::AddUserItemTree {
        parent,
        prev_sibling: engine.nodes.get(old).unwrap().node_data().prev_sibling,
        tree,
    });
    engine.apply_edits().map_err(|e| e.to_string())?;
    let new = engine.node_id_by_uuid(instance.0).unwrap();
    let old_placement = children::placement(engine, old)?.node_data().id;
    let new_placement = children::placement(engine, new)?.node_data().id;
    let output = children::placement(engine, old)?.output_parameter();
    engine.edits.push(Edit::RemoveNode { node: new_placement });
    engine.edits.push(Edit::MoveNode {
        node: old_placement,
        new_parent: new,
        new_prev_sibling: None,
    });
    engine.edits.push(Edit::RemoveNode { node: old });
    engine.edits.push(Edit::SetParam {
        node: output,
        value: ParamValue::Reference(
            source
                .channel
                .map_or_else(golden_core::node::NodeReference::empty, |channel| {
                    golden_core::node::NodeReference::new(channel.0)
                }),
        ),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.edits.push(Edit::PatchMeta {
        node: new,
        patch: golden_core::node::NodeMetaPatch {
            label: Some(label.clone()),
            ..Default::default()
        },
    });
    definition.source = source.channel.map(|channel| ChannelReference {
        media: MediaId(instance.0),
        channel,
    });
    Ok(MediaEditResult::InstanceChanged {
        snapshot: InstanceSnapshot {
            reference: InstanceReference {
                composition: reference,
                instance,
            },
            label,
            definition,
        },
    })
}
