//! References own a target and optional output resampling, with virtual target-channel identity.
mod node;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::node::Node;
pub use node::ReferenceNode;
use std::collections::{HashMap, HashSet};
pub(crate) const NODE_TYPE: &str = "mapgyver.media.reference.v3";

#[derive(Clone, Copy)]
pub(super) enum Target {
    Ready(MediaId),
    Unconfigured,
    Missing,
}

/// One immutable query memoizes chains; no document-wide channel materialization or callbacks.
pub(super) struct Resolver<'a> {
    engine: &'a AppEngine,
    leaves: HashMap<MediaId, Target>,
}
impl<'a> Resolver<'a> {
    pub fn new(engine: &'a AppEngine) -> Self {
        Self {
            engine,
            leaves: HashMap::new(),
        }
    }
    pub fn leaf(&mut self, media: MediaId) -> Result<Option<MediaId>, String> {
        Ok(match self.target(media)? {
            Target::Ready(media) => Some(media),
            Target::Unconfigured | Target::Missing => None,
        })
    }
    pub fn target(&mut self, mut media: MediaId) -> Result<Target, String> {
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let leaf = loop {
            if let Some(leaf) = self.leaves.get(&media) {
                break *leaf;
            }
            if !seen.insert(media) {
                return Err("Reference media dependency cycle".into());
            }
            path.push(media);
            match self
                .engine
                .node_id_by_uuid(media.0)
                .and_then(|id| self.engine.nodes.get(id))
            {
                Some(AppNode::Media(_)) => break Target::Ready(media),
                Some(AppNode::ReferenceMedia(node)) => match node.target() {
                    Some(target) => media = target,
                    None => break Target::Unconfigured,
                },
                None => break Target::Missing,
                _ => return Err("Reference target must be media".into()),
            }
        };
        for id in path {
            self.leaves.insert(id, leaf);
        }
        Ok(leaf)
    }
    pub fn canonical(&mut self, reference: ChannelReference) -> Result<ChannelReference, String> {
        Ok(self
            .leaf(reference.media)?
            .map_or(reference, |media| ChannelReference { media, ..reference }))
    }
    pub fn channels(&mut self, owner: MediaId) -> Result<Channels<'a>, String> {
        let next = self
            .leaf(owner)?
            .and_then(|leaf| self.engine.node_id_by_uuid(leaf.0))
            .and_then(|id| self.engine.nodes.get(id))
            .and_then(|node| node.node_data().first_child);
        Ok(Channels {
            engine: self.engine,
            next,
        })
    }
}

pub(super) struct Channels<'a> {
    engine: &'a AppEngine,
    next: Option<golden_core::node::NodeId>,
}
impl<'a> Iterator for Channels<'a> {
    type Item = &'a super::ChannelNode;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(id) = self.next {
            let node = self.engine.nodes.get(id)?;
            self.next = node.node_data().next_sibling;
            if let AppNode::MediaChannel(channel) = node
                && crate::matrix::outputs::exposed(self.engine, channel)
            {
                return Some(channel);
            }
        }
        None
    }
}

pub(super) fn channel(engine: &AppEngine, reference: ChannelReference) -> Result<ChannelSnapshot, String> {
    let leaf = Resolver::new(engine)
        .leaf(reference.media)?
        .ok_or("Reference target is missing or unconfigured")?;
    let source = super::authoring::channel_internal(
        engine,
        ChannelReference {
            media: leaf,
            ..reference
        },
    )?;
    let Some(AppNode::ReferenceMedia(node)) = engine
        .node_id_by_uuid(reference.media.0)
        .and_then(|id| engine.nodes.get(id))
    else {
        return Err("media is not Reference".into());
    };
    Ok(ChannelSnapshot {
        reference,
        label: source.label,
        definition: node.definition(engine, reference.channel),
    })
}

pub(super) fn create(
    engine: &mut AppEngine,
    parent: golden_core::node::NodeId,
    target: Option<MediaId>,
    requested: Option<String>,
) -> Result<MediaEditResult, String> {
    if let Some(target) = target {
        super::authoring::media(engine, target)?;
    }
    let label = super::authoring::label_with_reserved(engine, parent, requested, "Reference", &[])?;
    let node = ReferenceNode::from_target(label, target);
    let media = MediaId(node.node_data().meta.uuid);
    engine.edits.push(golden_core::edit::Edit::AddUserItemTree {
        parent,
        prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
        tree: golden_core::edit::NodeTree::new(node),
    });
    Ok(MediaEditResult::ReferenceCreated { media })
}

pub(super) fn configure(engine: &mut AppEngine, owner: MediaId, target: Option<MediaId>) -> Result<(), String> {
    let (id, _) = super::authoring::media(engine, owner)?;
    let Some(AppNode::ReferenceMedia(node)) = engine.nodes.get(id) else {
        return Err("media is not Reference".into());
    };
    let parameter = node.target_parameter();
    engine.edits.push(golden_core::edit::Edit::SetParam {
        node: parameter,
        value: golden_core::parameter::ParamValue::Reference(
            target.map_or_else(golden_core::node::NodeReference::empty, |id| {
                golden_core::node::NodeReference::new(id.0)
            }),
        ),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
    });
    Ok(())
}

#[cfg(test)]
mod tests;
