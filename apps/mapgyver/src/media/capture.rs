//! Capture only demanded authored leaves after backend transaction settlement. No plan
//! construction, whole-tree snapshot, device access or wait occurs on the engine owner.

use super::protocol::{AssetId, AssetRecord, ChannelDefinition, ChannelReference, ChannelSource};
use crate::app::{AppEngine, AppNode};
use golden_core::model::NodeUuid;
use golden_core::node::Node;
use golden_io::RevisionGuard;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct CapturedChannels {
    pub(crate) project: uuid::Uuid,
    pub(crate) guards: Arc<[RevisionGuard]>,
    pub(super) channels: Vec<CapturedChannel>,
    pub(super) assets: Vec<CapturedAsset>,
}

#[derive(Clone)]
pub(super) struct CapturedAsset {
    pub record: AssetRecord,
    pub guard: RevisionGuard,
}

#[derive(Clone)]
pub(super) struct CapturedChannel {
    pub reference: ChannelReference,
    pub definition: Option<ChannelDefinition>,
    pub enabled: bool,
}

impl CapturedChannels {
    pub fn is_current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}

pub fn capture_channels(
    engine: &AppEngine,
    references: &[ChannelReference],
    limit: usize,
) -> Result<CapturedChannels, String> {
    if references.len() > limit {
        return Err("channel capture exceeds admitted demand count".into());
    }
    let manager_id = super::authoring::manager(engine)?;
    let Some(AppNode::MediaManager(manager)) = engine.nodes.get(manager_id) else {
        unreachable!()
    };
    let mut guards = vec![manager.project_guard()];
    let mut seen = HashSet::new();
    let mut channels = Vec::new();
    let mut assets = Vec::new();
    let mut seen_assets = HashSet::<AssetId>::new();
    for &reference in references {
        if !seen.insert(reference) {
            continue;
        }
        let media = engine
            .node_id_by_uuid(reference.media.0)
            .and_then(|id| engine.nodes.get(id));
        let definition = match media {
            Some(AppNode::Media(media)) if media.node_data().parent == Some(manager_id) => {
                guards.push(media.guard());
                let channel = engine
                    .node_id_by_uuid(reference.channel.0)
                    .and_then(|id| engine.nodes.get(id));
                match channel {
                    Some(AppNode::MediaChannel(channel))
                        if channel.node_data().parent == Some(media.node_data().id) =>
                    {
                        if channel.definition().media_kind() != media.kind {
                            return Err("channel kind differs from owner".into());
                        }
                        guards.push(channel.guard());
                        Some((channel.definition().clone(), channel.node_data().effective_enabled))
                    }
                    _ => None,
                }
            }
            _ => {
                guards.push(manager.membership_guard());
                None
            }
        };
        let (definition, enabled) = match definition {
            Some((definition, enabled)) => (Some(definition), enabled),
            None => (None, false),
        };
        if enabled
            && let Some(ChannelDefinition {
                source: ChannelSource::Picture { asset: Some(asset) },
                ..
            }) = &definition
            && seen_assets.insert(*asset)
        {
            // Catalog membership also fences a source moved out through a generic edit.
            if seen_assets.len() == 1 {
                guards.push(manager.membership_guard());
            }
            let node = engine
                .node_id_by_uuid(NodeUuid(asset.get()))
                .and_then(|id| engine.nodes.get(id));
            match node {
                Some(AppNode::MediaAsset(node)) if node.node_data().parent == Some(manager_id) => {
                    let guard = node.guard();
                    guards.push(guard.clone());
                    if let Ok(record) = node.record() {
                        assets.push(CapturedAsset { record, guard });
                    }
                }
                _ => guards.push(manager.membership_guard()),
            }
        }
        channels.push(CapturedChannel {
            reference,
            definition,
            enabled,
        });
    }
    Ok(CapturedChannels {
        project: manager.instance,
        guards: Arc::from(guards),
        channels,
        assets,
    })
}
