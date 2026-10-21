//! Bounded authored route resets/defaults. A completion covers the fence and its
//! writes; flags settle only from a fresh graph after that completion resolves.
use golden_audio::PhysicalChannelKey;
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard, NodeTree},
    node::{Node, NodeId, NodeReference},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use super::{find_path, structure, SoundCardModule, INPUT_ROUTES_PATH, OUTPUT_ROUTES_PATH};
use crate::app::module_modules_audio_sound_card_schema::{SoundCardInputRoute, SoundCardOutputRoute};

mod lookup;
mod prune;

const RESET_TURN: usize = 16;
const PHYSICAL_KEY_BYTES: usize = 1024;
const CHANNEL_NAME_BYTES: usize = 64 * 1024;

#[derive(Default)]
pub(super) struct RouteWork {
    lifetime: golden_io::RevisionSource,
    lookup: lookup::Lookup,
    prune_input: prune::Pruning,
    prune_output: prune::Pruning,
    input: DirectionWork,
    output: DirectionWork,
}

#[derive(Default)]
struct DirectionWork {
    reset: Option<EditCompletion>,
    seed: Option<EditCompletion>,
}

impl DirectionWork {
    fn reset(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        path: &str,
        budget: &mut usize,
        lifetime: golden_io::RevisionGuard,
    ) -> Result<bool, String> {
        // An older accepted seed must settle before the new selection clears it.
        if pending(&self.reset) || pending(&self.seed) {
            return Ok(true);
        }
        self.reset = None;
        self.seed = None;
        let parent = find_path(snapshot, module, path).ok_or("Sound Card route folder is missing")?;
        let children = snapshot.child_ids_slice(parent);
        if children.is_empty() {
            return Ok(false);
        }
        if *budget == 0 {
            return Ok(true);
        }
        let count = children.len().min(*budget);
        let mut guards = Vec::with_capacity(RESET_TURN + 2);
        guards.push(identity(snapshot, module)?);
        guards.push(identity(snapshot, parent)?);
        let mut edits = EditQueue::new();
        for &route in &children[..count] {
            guards.push(identity(snapshot, route)?);
        }
        edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(guards, vec![lifetime]).expect("bounded route read set"),
        });
        for &node in &children[..count] {
            edits.push(Edit::RemoveNode { node });
        }
        *budget -= count;
        self.reset = Some(edits.completion());
        ctx.edits.pending.extend(edits.drain());
        Ok(true)
    }

    fn seed(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        direction: &'static str,
        physical: &[PhysicalChannelKey],
        lifetime: golden_io::RevisionGuard,
    ) -> Result<SeedProgress, String> {
        if pending(&self.seed) {
            return Ok(SeedProgress::Pending);
        }
        self.seed = None;
        if physical.is_empty() {
            return Ok(SeedProgress::AwaitingChannels);
        }
        let namespace = snapshot.node(module).ok_or("Sound Card module is missing")?.uuid;
        let routes = if direction == "input" {
            INPUT_ROUTES_PATH
        } else {
            OUTPUT_ROUTES_PATH
        };
        let parent = find_path(snapshot, module, routes).ok_or("Sound Card route folder is missing")?;
        let wanted = structure::direction_channel_count(
            snapshot,
            module,
            &format!("connection/{direction}_routing/channel_count"),
        )
        .min(physical.len())
        .min(2);
        let mut targets = Vec::with_capacity(2);
        for (index, key) in physical.iter().take(wanted).enumerate() {
            if key.as_str().len() > PHYSICAL_KEY_BYTES {
                return Err("Sound Card default-route physical key exceeds 1024 bytes".into());
            }
            let Some(reference) = default_channel_reference(snapshot, module, direction, index + 1)? else {
                return Ok(SeedProgress::AwaitingChannels);
            };
            let uuid = reference.uuid();
            let route_uuid = structure::route_uuid(namespace, direction, key.as_str(), uuid);
            if let Some(existing) = snapshot.node_id_by_uuid(route_uuid).and_then(|id| snapshot.node(id)) {
                let expected = if direction == "input" {
                    SoundCardInputRoute::NODE_TYPE
                } else {
                    SoundCardOutputRoute::NODE_TYPE
                };
                if existing.parent != Some(parent) || existing.node_type != expected {
                    return Err("Sound Card default-route identity is already owned by another node".into());
                }
                if ["physical_channel", "channel"].iter().any(|path| {
                    find_path(snapshot, existing.id, path)
                        .and_then(|id| snapshot.node(id))
                        .and_then(|node| node.param_value.as_ref())
                        .is_none()
                }) {
                    return Ok(SeedProgress::AwaitingChannels);
                }
                continue;
            }
            targets.push((reference, key));
        }
        if targets.is_empty() {
            return Ok(SeedProgress::Complete);
        }
        let mut guards = Vec::with_capacity(4);
        guards.push(identity(snapshot, module)?);
        guards.push(identity(snapshot, parent)?);
        let mut edits = EditQueue::new();
        for (channel, _) in &targets {
            guards.push(
                NodeReadGuard::capture(snapshot, channel.cached_id().expect("indexed channel"))
                    .ok_or("Sound Card default channel retired")?,
            );
        }
        edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(guards, vec![lifetime]).expect("bounded default read set"),
        });
        for (reference, key) in targets {
            let uuid = reference.uuid();
            let mut tree = if direction == "input" {
                NodeTree::new(SoundCardInputRoute::connected(key.as_str().to_owned(), reference))
            } else {
                NodeTree::new(SoundCardOutputRoute::connected(reference, key.as_str().to_owned()))
            };
            structure::set_route_identity(tree.node.as_mut(), namespace, direction, key.as_str(), uuid);
            edits.push(Edit::AddNodeTree {
                parent,
                tree,
                prev_sibling: None,
            });
        }
        self.seed = Some(edits.completion());
        ctx.edits.pending.extend(edits.drain());
        Ok(SeedProgress::Pending)
    }
}

enum SeedProgress {
    Pending,
    Complete,
    AwaitingChannels,
}

impl RouteWork {
    pub(super) fn lookup_needs_snapshot(&self) -> bool {
        self.lookup.needs_snapshot()
    }
    pub(super) fn pruning(&self) -> bool {
        self.prune_input.busy() || self.prune_output.busy()
    }
    pub(super) fn prune_needs_snapshot(&self) -> bool {
        self.prune_input.needs_snapshot() || self.prune_output.needs_snapshot()
    }
    pub(super) fn prune_waiting(&self) -> bool {
        self.prune_input.waiting() || self.prune_output.waiting()
    }
    pub(super) fn seeding_input(&self) -> bool {
        pending(&self.input.seed)
    }
    pub(super) fn seeding_output(&self) -> bool {
        pending(&self.output.seed)
    }
    pub(super) fn invalidate(&mut self) {
        self.lifetime.advance();
        self.lookup = lookup::Lookup::default();
        self.prune_input = prune::Pruning::default();
        self.prune_output = prune::Pruning::default();
    }
    fn guard(&self) -> golden_io::RevisionGuard {
        let reader = self.lifetime.reader();
        reader.guard_at(reader.current())
    }
}

pub(super) fn default_channel_reference(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    direction: &str,
    ordinal: usize,
) -> Result<Option<NodeReference>, String> {
    let namespace = snapshot.node(module).ok_or("Sound Card module is missing")?.uuid;
    let parent = find_path(snapshot, module, &format!("parameters/{direction}/channels"))
        .ok_or("Sound Card channel folder is missing")?;
    let uuid = structure::derived_uuid(namespace, format!("{direction}-channel-{ordinal}").as_bytes());
    let Some(channel) = snapshot.node_id_by_uuid(uuid).and_then(|id| snapshot.node(id)) else {
        return Ok(None);
    };
    if channel.parent != Some(parent) || !matches!(channel.param_value, Some(ParamValue::Float(_))) {
        return Ok(None);
    }
    if channel.short_name.len() > CHANNEL_NAME_BYTES || channel.label.len() > CHANNEL_NAME_BYTES {
        return Err("Sound Card default-route channel name exceeds 64 KiB".into());
    }
    let mut reference = NodeReference::with_cached_id(uuid, Some(channel.id));
    reference.set_cached_name(Some(structure::channel_name(channel)));
    Ok(Some(reference))
}

fn pending(receipt: &Option<EditCompletion>) -> bool {
    receipt
        .as_ref()
        .is_some_and(|receipt| receipt.status() == EditCompletionStatus::Pending)
}

fn identity(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Result<NodeReadGuard, String> {
    NodeReadGuard::capture_identity(snapshot, node).ok_or_else(|| "Sound Card route target retired".into())
}

impl SoundCardModule {
    pub(super) fn require_route_authoring_ready(
        &self,
        direction: golden_audio::AudioDirection,
    ) -> Result<(), golden_audio::AudioError> {
        let pending = match direction {
            golden_audio::AudioDirection::Input => self.input_route_reset_pending || pending(&self.routes.input.seed),
            golden_audio::AudioDirection::Output => {
                self.output_route_reset_pending || pending(&self.routes.output.seed)
            }
        };
        if pending {
            return Err(super::integration::command_error(
                "Sound Card routes are being reconciled; retry the routing intent after completion",
            ));
        }
        Ok(())
    }

    pub(super) fn reset_routes_for_device_selection(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<bool, String> {
        // Backpressure the reset cohort until its previously accepted turn settles.
        // Repeated service must not admit another direction from the same stale view.
        if (self.input_route_reset_pending && (pending(&self.routes.input.reset) || pending(&self.routes.input.seed)))
            || (self.output_route_reset_pending
                && (pending(&self.routes.output.reset) || pending(&self.routes.output.seed)))
        {
            self.wake_route_work();
            return Ok(true);
        }
        if (self.input_route_reset_pending || self.output_route_reset_pending) && self.routes.lookup.pending() {
            self.wake_route_work();
            return Ok(true);
        }
        if (self.input_route_reset_pending && self.routes.prune_input.pending())
            || (self.output_route_reset_pending && self.routes.prune_output.pending())
        {
            self.wake_route_work();
            return Ok(true);
        }
        if self.input_route_reset_pending {
            self.routes.prune_input = prune::Pruning::default();
        }
        if self.output_route_reset_pending {
            self.routes.prune_output = prune::Pruning::default();
        }
        let mut budget = RESET_TURN;
        let mut busy = false;
        let mut error = None;
        let lifetime = self.routes.guard();
        for (work, requested, path) in [
            (
                &mut self.routes.input,
                &mut self.input_route_reset_pending,
                INPUT_ROUTES_PATH,
            ),
            (
                &mut self.routes.output,
                &mut self.output_route_reset_pending,
                OUTPUT_ROUTES_PATH,
            ),
        ] {
            if *requested {
                match work.reset(ctx, snapshot, self.base.id(), path, &mut budget, lifetime.clone()) {
                    Ok(pending) => {
                        *requested = pending;
                        busy |= pending;
                    }
                    Err(rejected) => error = Some(rejected),
                }
            }
        }
        if busy {
            self.wake_route_work();
        }
        if busy {
            Ok(true)
        } else {
            error.map_or(Ok(false), Err)
        }
    }

    /// Only fresh-project or explicit selection-change requests seed defaults.
    pub(super) fn seed_default_routes_from_inventory(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        input: &[PhysicalChannelKey],
        output: &[PhysicalChannelKey],
    ) -> Result<bool, String> {
        if self.reset_routes_for_device_selection(ctx, snapshot)? {
            return Ok(true);
        }
        let mut busy = false;
        let mut error = None;
        let lifetime = self.routes.guard();
        for (work, requested, direction, physical) in [
            (
                &mut self.routes.input,
                &mut self.input_default_routes_pending,
                "input",
                input,
            ),
            (
                &mut self.routes.output,
                &mut self.output_default_routes_pending,
                "output",
                output,
            ),
        ] {
            if *requested {
                match work.seed(ctx, snapshot, self.base.id(), direction, physical, lifetime.clone()) {
                    Ok(SeedProgress::Pending) => busy = true,
                    Ok(SeedProgress::Complete) => *requested = false,
                    Ok(SeedProgress::AwaitingChannels) => {}
                    Err(rejected) => error = Some(rejected),
                }
            }
        }
        if busy {
            self.wake_route_work();
        }
        if busy {
            Ok(true)
        } else {
            error.map_or(Ok(false), Err)
        }
    }

    fn wake_route_work(&self) {
        if let Some(wake) = &self.runtime_wake {
            wake.wake();
        }
    }
}

#[cfg(test)]
mod tests;
