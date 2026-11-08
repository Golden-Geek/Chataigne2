//! Capture bounded demanded authored dependencies after backend transaction settlement. No plan
//! construction, whole-tree snapshot, device access or wait occurs on the engine owner.

use super::composition::protocol::InstanceDefinition;
use super::protocol::{AssetId, AssetRecord, ChannelDefinition, ChannelReference, ChannelSource};
use crate::app::{AppEngine, AppNode};
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
    sources: Arc<super::retention::RuntimeSources>,
    pub(super) resolutions: Arc<super::resolution::RuntimeResolutions>,
    source_guard: RevisionGuard,
}

#[derive(Clone)]
pub(super) struct CapturedAsset {
    pub record: AssetRecord,
    pub reload: u64,
}

#[derive(Clone)]
pub(super) struct CapturedChannel {
    pub video: Option<Arc<super::video::VideoControl>>,
    pub picture_reload: u64,
    pub public_output: bool,
    pub reference: ChannelReference,
    pub definition: Option<ChannelDefinition>,
    pub enabled: bool,
    /// Only actual Cell-owned media may outlive removal; external target deletion stays missing.
    pub owned: bool,
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
            .chain(self.definition.iter().filter_map(|definition| match definition.source {
                ChannelSource::Reference { target } => target,
                _ => None,
            }))
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
                ChannelSource::Composition { .. } | ChannelSource::Matrix { .. } | ChannelSource::Reference { .. }
            )
        })
    }
}

impl CapturedChannels {
    /// Bounded source diagnostics follow canonical References and editor dependencies.
    pub(crate) fn video_source_status(
        &self,
        mut reference: ChannelReference,
    ) -> Option<super::video::protocol::VideoSourceStatus> {
        for _ in 0..self.channels.len() {
            let channel = self.channels.iter().find(|channel| channel.reference == reference)?;
            if !channel.enabled {
                return None;
            }
            match &channel.definition.as_ref()?.source {
                ChannelSource::Reference { target: Some(target) } => reference = *target,
                ChannelSource::Video { file } => return channel.video.as_ref()?.source_status(reference, file).ok(),
                _ => return None,
            }
        }
        None
    }
    /// Whole captured Matrix preparation/active/outgoing demand, including clamped neighbors.
    /// This reads immutable metadata only; reconcile it on a live application runtime explicitly.
    pub fn application_demand(&self) -> Vec<crate::applications::demand::ApplicationDemand> {
        let mut result = std::collections::HashMap::<_, crate::applications::demand::ApplicationDemand>::new();
        for entry in self
            .channels
            .iter()
            .filter_map(|channel| channel.matrix.as_ref())
            .flat_map(|matrix| matrix.applications.demand.iter())
        {
            result
                .entry(entry.application)
                .and_modify(|value| value.reasons.extend(&entry.reasons))
                .or_insert_with(|| entry.clone());
        }
        let mut result = result.into_values().collect::<Vec<_>>();
        result.sort_by_key(|entry| entry.application.0.0);
        result
    }
    pub(crate) fn plan_applications(&self) -> Vec<crate::applications::protocol::ApplicationId> {
        let mut references = self
            .channels
            .iter()
            .filter_map(|channel| channel.matrix.as_ref())
            .flat_map(|matrix| matrix.applications.references.iter())
            .copied()
            .collect::<Vec<_>>();
        references.sort_by_key(|id| id.0.0);
        references.dedup();
        references
    }
    pub(crate) fn transitions_finished(&self, time: golden_time::TimeSnapshot) -> bool {
        self.channels
            .iter()
            .filter_map(|channel| channel.matrix.as_ref())
            .any(|matrix| matrix.until.is_some_and(|until| until.finished(time)))
    }
    pub(crate) fn settle_transitions(&mut self, time: golden_time::TimeSnapshot) -> Result<(), String> {
        for matrix in self.channels.iter_mut().filter_map(|channel| channel.matrix.as_mut()) {
            matrix.settle(time)?;
        }
        self.sources.prune();
        self.source_guard = self.sources.guard();
        Ok(())
    }
    pub fn is_current(&self) -> bool {
        self.revision_guards().all(RevisionGuard::is_current)
    }

    pub(crate) fn base_guards(&self) -> impl Iterator<Item = &RevisionGuard> {
        std::iter::once(&self.source_guard).chain(self.guards.iter())
    }
    pub(crate) fn revision_guards(&self) -> impl Iterator<Item = &RevisionGuard> {
        self.base_guards()
            .chain(self.channels.iter().flat_map(|channel| &channel.instance_guards))
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
    let sources = &manager.sources;
    let retired = sources.snapshot();
    let publicly_exposed = |reference: ChannelReference| {
        if engine.node_id_by_uuid(reference.channel.0).is_some() {
            super::visibility::exposed(engine, reference)
        } else {
            retired.get(&reference).is_none_or(|channel| channel.public_output)
        }
    };
    let mut seen = HashSet::new();
    let mut channels = Vec::new();
    let mut assets = Vec::new();
    let mut seen_assets = HashSet::<AssetId>::new();
    let mut guarded_matrices = HashSet::new();
    let mut retained_matrices = HashSet::new();
    let mut guarded_media = HashSet::new();
    let mut pending = std::collections::VecDeque::new();
    let mut hidden_roots = HashSet::new();
    for &reference in references {
        if let Some(guard) = super::visibility::guard(engine, reference) {
            guards.push(guard);
        }
        if !publicly_exposed(reference) {
            hidden_roots.insert(reference);
            continue;
        }
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
        let frozen = media
            .is_none()
            .then(|| retired.get(&reference).filter(|channel| channel.owned))
            .flatten();
        let picture_reload = engine
            .node_id_by_uuid(reference.channel.0)
            .and_then(|id| engine.nodes.get(id))
            .and_then(|node| match node {
                AppNode::MediaChannel(channel) => Some(channel.picture_reload_generation()),
                _ => None,
            })
            .unwrap_or_else(|| frozen.map_or(0, |channel| channel.picture_reload));
        let definition = match media {
            Some(AppNode::ReferenceMedia(node))
                if super::folders::contains_parent(engine, node.node_data().parent, manager_id)
                    && node.output_reference(engine) == reference =>
            {
                guards.push(node.guard());
                guards.push(manager.membership_guard());
                Some((
                    node.definition(engine, reference.channel),
                    node.node_data().effective_enabled,
                ))
            }
            Some(AppNode::Media(media))
                if super::folders::contains_parent(engine, media.node_data().parent, manager_id) =>
            {
                let mut parent = media.node_data().parent;
                while let Some(AppNode::MediaFolder(folder)) = parent.and_then(|id| engine.nodes.get(id)) {
                    guards.push(folder.guard());
                    parent = folder.node_data().parent;
                }
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
                        if channel.kind() != media.kind() {
                            return Err("channel kind differs from owner".into());
                        }
                        guards.push(channel.guard());
                        Some((
                            super::authoring::effective_definition(engine, channel.as_ref())?,
                            channel.node_data().effective_enabled,
                        ))
                    }
                    _ => None,
                }
            }
            _ => {
                guards.push(manager.membership_guard());
                frozen.and_then(|channel| {
                    channel
                        .definition
                        .clone()
                        .map(|definition| (definition, channel.enabled))
                })
            }
        };
        let (mut definition, enabled) = match definition {
            Some((definition, enabled)) => (Some(definition), enabled),
            None => (None, false),
        };
        if let Some(ChannelDefinition {
            source: ChannelSource::Reference { target },
            ..
        }) = &mut definition
            && target.is_some_and(|target| !publicly_exposed(target))
        {
            if let Some(guard) = target.and_then(|target| super::visibility::guard(engine, target)) {
                guards.push(guard);
            }
            *target = None;
        }
        let mut instances = frozen.and_then(|channel| channel.instances.clone());
        let mut instance_guards = Vec::new();
        let mut matrix = frozen.and_then(|channel| channel.matrix.clone());
        if let (Some(matrix), Some(time)) = (&mut matrix, time) {
            matrix.settle(time)?;
        }
        if frozen.is_none()
            && enabled
            && definition
                .as_ref()
                .is_some_and(|definition| matches!(definition.source, ChannelSource::Composition { .. }))
        {
            let channel = engine
                .node_id_by_uuid(reference.channel.0)
                .and_then(|id| engine.nodes.get(id))
                .unwrap();
            let owner = channel.node_data().parent.ok_or("Composition has no media")?;
            let mut next = engine
                .nodes
                .get(owner)
                .ok_or("Composition media is missing")?
                .node_data()
                .first_child;
            let mut active = Vec::new();
            let mut admitted = true;
            while let Some(id) = next {
                if let Some(node) = engine.nodes.get(id)
                    && (crate::parameters::is_parameter_folder(node)
                        || matches!(node, AppNode::MediaChannel(_) | AppNode::CompositionPlacement(_)))
                {
                    next = node.node_data().next_sibling;
                    continue;
                }
                if instance_budget == 0 {
                    admitted = false;
                    break;
                }
                instance_budget -= 1;
                let node = engine.nodes.get(id).ok_or("Composition media child is missing")?;
                if !matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
                    admitted = false;
                    break;
                }
                next = node.node_data().next_sibling;
                let placement = super::composition::children::placement(engine, id)?;
                instance_guards.push(placement.guard());
                let mut definition = super::composition::children::definition(engine, id)?;
                if node.node_data().effective_enabled && definition.enabled && definition.opacity > 0.0 {
                    definition.source = super::MediaSource {
                        media: super::protocol::MediaId(node.node_data().meta.uuid),
                        channel: placement.output(),
                    }
                    .capture(engine, &mut instance_guards)?;
                }
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
        if frozen.is_none()
            && enabled
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
                if retained_matrices.insert(reference.media)
                    && let Some(control) = &matrix.control
                {
                    control.bind_sources(sources, reference.media)?;
                }
                for source in matrix.inputs.iter().map(|input| input.source) {
                    if seen.insert(source) {
                        pending.push_back(source);
                    }
                }
            }
        }
        if frozen.is_some() {
            let elements = instances
                .as_ref()
                .map_or(0, Vec::len)
                .saturating_add(matrix.as_ref().map_or(0, |matrix| matrix.inputs.len()));
            if elements > instance_budget {
                instances = None;
                matrix = None;
            } else {
                instance_budget -= elements;
            }
            let dependencies = instances
                .iter()
                .flatten()
                .filter_map(|instance| instance.source)
                .chain(
                    matrix
                        .iter()
                        .flat_map(|matrix| matrix.inputs.iter().map(|input| input.source)),
                )
                .collect::<Vec<_>>();
            let additions = dependencies
                .iter()
                .filter(|source| !seen.contains(source))
                .copied()
                .collect::<HashSet<_>>();
            if seen.len().saturating_add(additions.len()) > limit {
                instances = None;
                matrix = None;
            } else {
                for dependency in dependencies {
                    if seen.insert(dependency) {
                        pending.push_back(dependency);
                    }
                }
            }
        }
        if enabled
            && let Some(ChannelDefinition {
                source: ChannelSource::Reference { target: Some(target) },
                ..
            }) = &definition
            && (seen.contains(target) || seen.len() < limit)
            && seen.insert(*target)
        {
            pending.push_back(*target);
        }
        if enabled
            && let Some(ChannelDefinition {
                source: ChannelSource::Picture { file: Some(file) },
                ..
            }) = &definition
            && seen_assets.insert(AssetId::from_uuid(reference.channel.0.0))
        {
            // The File is already captured in the channel definition. Its resource address is
            // runtime metadata, including when a playing owned source outlives authored removal.
            assets.push(CapturedAsset {
                reload: picture_reload,
                record: AssetRecord {
                    id: AssetId::from_uuid(reference.channel.0.0),
                    locator: file.clone(),
                },
            });
        }
        let mut ancestor = media.and_then(|node| node.node_data().parent);
        let mut owned = frozen.is_some();
        while let Some(node) = ancestor.and_then(|id| engine.nodes.get(id)) {
            if matches!(node, AppNode::MatrixCell(_)) {
                owned = true;
                break;
            }
            ancestor = node.node_data().parent;
        }
        channels.push(CapturedChannel {
            video: match media {
                Some(AppNode::Media(node)) => node.video_control(),
                _ => frozen.and_then(|channel| channel.video.clone()),
            },
            picture_reload,
            public_output: publicly_exposed(reference),
            owned,
            reference,
            definition,
            enabled,
            instances,
            instance_guards,
            matrix,
        });
    }
    for reference in hidden_roots.difference(&seen) {
        channels.push(CapturedChannel {
            video: None,
            picture_reload: 0,
            reference: *reference,
            public_output: false,
            definition: None,
            enabled: false,
            owned: false,
            instances: None,
            instance_guards: vec![],
            matrix: None,
        });
    }
    sources.update(&channels)?;
    let mut captured = CapturedChannels {
        source_guard: sources.guard(),
        sources: sources.clone(),
        resolutions: manager.resolutions.clone(),
        project: manager.instance,
        guards: Arc::from(guards),
        channels,
        assets,
    };
    let demand = captured.application_demand();
    if !demand.is_empty() {
        let mut guards = captured.guards.to_vec();
        let mut availability = std::collections::HashMap::new();
        guards.push(manager.membership_guard());
        for entry in demand {
            if let Some(node) = crate::applications::control_node(engine, entry.application) {
                guards.push(node.guard());
                availability.insert(entry.application, node.availability());
            }
        }
        captured.guards = guards.into();
        for entry in captured
            .channels
            .iter_mut()
            .filter_map(|channel| channel.matrix.as_mut())
            .flat_map(|matrix| Arc::make_mut(&mut matrix.applications.demand).iter_mut())
        {
            if let Some(available) = availability.get(&entry.application) {
                entry.availability = *available;
            }
        }
    }
    Ok(captured)
}
