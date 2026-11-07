//! One stable Reference-owned output selects one authored source channel.
mod node;
mod output;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::node::Node;
pub use node::ReferenceNode;
pub(crate) use output::OUTPUT_TYPE;
pub use output::ReferenceOutputNode;
use std::collections::{HashMap, HashSet};
pub(crate) const NODE_TYPE: &str = "mapgyver.media.reference.v4";

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
    canonical: HashMap<ChannelReference, ChannelReference>,
}
impl<'a> Resolver<'a> {
    pub fn new(engine: &'a AppEngine) -> Self {
        Self {
            engine,
            leaves: HashMap::new(),
            canonical: HashMap::new(),
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
                Some(AppNode::ReferenceMedia(node)) => match node.source(self.engine) {
                    Some(target) => {
                        if super::authoring::channel_internal(self.engine, target).is_err() {
                            break Target::Missing;
                        }
                        media = target.media;
                    }
                    None if node
                        .target()
                        .is_some_and(|target| self.engine.node_id_by_uuid(target.0).is_none()) =>
                    {
                        break Target::Missing;
                    }
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
        let mut current = reference;
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let result = loop {
            if let Some(canonical) = self.canonical.get(&current) {
                break *canonical;
            }
            if !seen.insert(current) {
                return Err("Reference media dependency cycle".into());
            }
            path.push(current);
            let Some(AppNode::ReferenceMedia(node)) = self
                .engine
                .node_id_by_uuid(current.media.0)
                .and_then(|id| self.engine.nodes.get(id))
            else {
                break current;
            };
            if node.output_reference(self.engine) != current {
                break current;
            }
            let Some(source) = node.source(self.engine) else {
                break current;
            };
            current = source;
        };
        for source in path {
            self.canonical.insert(source, result);
        }
        Ok(result)
    }
    pub fn channels(&mut self, owner: MediaId) -> Result<Channels<'a>, String> {
        let next = self
            .engine
            .node_id_by_uuid(owner.0)
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
    type Item = &'a dyn Node;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(id) = self.next {
            let node = self.engine.nodes.get(id)?;
            self.next = node.node_data().next_sibling;
            match node {
                AppNode::MediaChannel(channel) if crate::matrix::outputs::exposed(self.engine, channel) => {
                    return Some(channel.as_ref());
                }
                AppNode::ReferenceOutput(output) => return Some(output.as_ref()),
                _ => {}
            }
        }
        None
    }
}

pub(super) fn channel(engine: &AppEngine, reference: ChannelReference) -> Result<ChannelSnapshot, String> {
    let Some(AppNode::ReferenceMedia(node)) = engine
        .node_id_by_uuid(reference.media.0)
        .and_then(|id| engine.nodes.get(id))
    else {
        return Err("media is not Reference".into());
    };
    if node.output_reference(engine) != reference {
        return Err("channel belongs to another media".into());
    }
    Ok(ChannelSnapshot {
        reference,
        label: "Output".into(),
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
    let source = target
        .map(|target| super::source_selection(engine, target.0))
        .transpose()?;
    let node = ReferenceNode::from_source(label, source);
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
