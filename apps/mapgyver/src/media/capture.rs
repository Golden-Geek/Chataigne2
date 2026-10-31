//! Capture bounded demanded authored dependencies after backend transaction settlement. No plan
//! construction, whole-tree snapshot, device access or wait occurs on the engine owner.

use super::composition::protocol::InstanceDefinition;
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
    /// Admitted active Composition instances; None for leaves or rejected instance admission.
    pub instances: Option<Vec<InstanceDefinition>>,
    pub instance_guards: Vec<RevisionGuard>,
    pub matrix: Option<crate::matrix::capture::CapturedMatrix>,
}

impl CapturedChannel {
    pub fn dependencies(&self) -> impl Iterator<Item = ChannelReference> + '_ {
        self.instances
            .iter()
            .flatten()
            .filter_map(|instance| instance.source)
            .chain(
                self.matrix
                    .iter()
                    .flat_map(|matrix| matrix.inputs.iter().map(|input| input.source)),
            )
    }
    pub fn derived(&self) -> bool {
        self.definition.as_ref().is_some_and(|definition| {
            matches!(
                definition.source,
                ChannelSource::Composition { .. } | ChannelSource::Matrix { .. }
            )
        })
    }
}

impl CapturedChannels {
    pub(crate) fn transitions_finished(&self, time: golden_time::TimeSnapshot) -> bool {
        self.channels
            .iter()
            .filter_map(|channel| channel.matrix.as_ref())
            .any(|matrix| matrix.until.is_some_and(|until| until.finished(time)))
    }
    pub(crate) fn settle_transitions(&mut self, time: golden_time::TimeSnapshot) {
        for matrix in self.channels.iter_mut().filter_map(|channel| channel.matrix.as_mut()) {
            if matrix.until.is_some_and(|until| until.finished(time)) {
                matrix.control.settle(time);
                matrix
                    .inputs
                    .retain(|input| input.weight.is_none_or(|weight| weight.target().get() > 0.0));
                for input in &mut matrix.inputs {
                    input.weight = None;
                }
                matrix.until = None;
                matrix.observation.transitioning = false;
                matrix
                    .observation
                    .columns
                    .retain(|column| Some(*column) == matrix.observation.playback.desired_column);
            }
        }
    }
    pub fn is_current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
            && self
                .channels
                .iter()
                .flat_map(|channel| &channel.instance_guards)
                .all(RevisionGuard::is_current)
    }
}

pub fn capture_channels(
    engine: &AppEngine,
    references: &[ChannelReference],
    limit: usize,
) -> Result<CapturedChannels, String> {
    capture_channels_with_time(engine, references, limit, None)
}

/// Captures timed Matrix demand using the same authority observed by the owning video runtime.
pub fn capture_channels_at(
    engine: &AppEngine,
    references: &[ChannelReference],
    limit: usize,
    time: golden_time::TimeSnapshot,
) -> Result<CapturedChannels, String> {
    capture_channels_with_time(engine, references, limit, Some(time))
}

pub(crate) fn capture_channels_with_time(
    engine: &AppEngine,
    references: &[ChannelReference],
    limit: usize,
    time: Option<golden_time::TimeSnapshot>,
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
    let mut guarded_matrices = HashSet::new();
    let mut guarded_media = HashSet::new();
    let mut pending = std::collections::VecDeque::new();
    for &reference in references {
        if seen.insert(reference) {
            pending.push_back(reference);
        }
    }
    // Authored graphs may be much larger than one active renderer scene.
    let mut instance_budget = 4096usize;
    while let Some(reference) = pending.pop_front() {
        let media = engine
            .node_id_by_uuid(reference.media.0)
            .and_then(|id| engine.nodes.get(id));
        let definition = match media {
            Some(AppNode::Media(media)) if media.node_data().parent == Some(manager_id) => {
                if guarded_media.insert(reference.media) {
                    guards.push(media.guard());
                }
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
        let mut instances = None;
        let mut instance_guards = Vec::new();
        let mut matrix = None;
        if enabled
            && definition
                .as_ref()
                .is_some_and(|definition| matches!(definition.source, ChannelSource::Composition { .. }))
        {
            let channel = engine
                .node_id_by_uuid(reference.channel.0)
                .and_then(|id| engine.nodes.get(id))
                .unwrap();
            let mut next = channel.node_data().first_child;
            let mut active = Vec::new();
            let mut admitted = true;
            while let Some(id) = next {
                if instance_budget == 0 {
                    admitted = false;
                    break;
                }
                instance_budget -= 1;
                let Some(AppNode::CompositionInstance(node)) = engine.nodes.get(id) else {
                    admitted = false;
                    break;
                };
                next = node.node_data().next_sibling;
                instance_guards.push(node.guard());
                let definition = node.definition();
                if node.node_data().effective_enabled
                    && definition.enabled
                    && definition.opacity > 0.0
                    && definition.source.is_some()
                {
                    active.push(definition.clone());
                }
            }
            let additions = active
                .iter()
                .filter_map(|instance| instance.source)
                .filter(|source| !seen.contains(source))
                .collect::<HashSet<_>>();
            if seen.len().saturating_add(additions.len()) > limit {
                admitted = false;
            }
            if admitted {
                for source in active.iter().filter_map(|instance| instance.source) {
                    if seen.insert(source) {
                        pending.push_back(source);
                    }
                }
                instances = Some(active);
            }
        }
        if enabled
            && let Some(ChannelDefinition {
                source: ChannelSource::Matrix { output },
                ..
            }) = &definition
        {
            matrix = crate::matrix::capture::capture(
                engine,
                reference,
                output,
                &mut instance_budget,
                &mut guarded_matrices,
                &mut guards,
                time,
            )?;
            let additions = matrix
                .iter()
                .flat_map(|matrix| matrix.inputs.iter().map(|input| input.source))
                .filter(|source| !seen.contains(source))
                .collect::<HashSet<_>>();
            if seen.len().saturating_add(additions.len()) > limit {
                matrix = None;
            }
            if let Some(matrix) = &matrix {
                for source in matrix.inputs.iter().map(|input| input.source) {
                    if seen.insert(source) {
                        pending.push_back(source);
                    }
                }
            }
        }
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
            instances,
            instance_guards,
            matrix,
        });
    }
    Ok(CapturedChannels {
        project: manager.instance,
        guards: Arc::from(guards),
        channels,
        assets,
    })
}
