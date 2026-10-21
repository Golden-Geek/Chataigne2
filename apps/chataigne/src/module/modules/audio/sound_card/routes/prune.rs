use super::*;
use golden_io::RetainedPayload;
use std::sync::Arc;

const SCAN_TURN: usize = 32;
const DELETE_TURN: usize = 8;

#[derive(Default)]
pub(super) struct Pruning(Option<Sweep>, Option<std::time::Instant>);

struct Sweep {
    source: RetainedPayload<ProcessTreeSnapshot>,
    parent: NodeId,
    desired: Vec<golden_core::node::NodeUuid>,
    cursor: usize,
    receipt: Option<(EditCompletion, usize)>,
}

impl Pruning {
    pub(super) fn waiting(&self) -> bool {
        self.1.is_some_and(|at| std::time::Instant::now() < at)
    }
    pub(super) fn pending(&self) -> bool {
        self.0
            .as_ref()
            .and_then(|s| s.receipt.as_ref())
            .is_some_and(|(r, _)| r.status() == EditCompletionStatus::Pending)
    }
    pub(super) fn busy(&self) -> bool {
        self.0.is_some() || self.1.is_some()
    }
    pub(super) fn needs_snapshot(&self) -> bool {
        self.0.is_some() || self.1.is_some_and(|at| std::time::Instant::now() >= at)
    }

    pub(super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &Arc<ProcessTreeSnapshot>,
        parent: NodeId,
        desired: &[NodeReference],
        guards: Vec<NodeReadGuard>,
        lifetime: golden_io::RevisionGuard,
    ) -> Result<bool, String> {
        if self.1.is_some_and(|at| std::time::Instant::now() < at) {
            return Ok(true);
        }
        self.1 = None;
        if self
            .0
            .as_ref()
            .and_then(|s| s.receipt.as_ref())
            .is_some_and(|(r, _)| r.status() == EditCompletionStatus::Pending)
        {
            return Ok(true);
        }
        let changed = self.0.as_ref().is_some_and(|s| {
            s.parent != parent
                || s.source.node(parent).map(|n| n.uuid) != snapshot.node(parent).map(|n| n.uuid)
                || s.desired.len() != desired.len()
                || s.desired.iter().zip(desired).any(|(a, b)| *a != b.uuid())
        });
        if changed {
            self.0 = None;
        }
        if self.0.is_none() {
            if snapshot.child_ids_slice(parent).is_empty() {
                return Ok(false);
            }
            let source = snapshot.try_lease().map_err(|_| {
                self.1 = Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
                "Sound Card prune snapshot capacity exhausted"
            })?;
            self.0 = Some(Sweep {
                source,
                parent,
                desired: desired.iter().take(256).map(NodeReference::uuid).collect(),
                cursor: 0,
                receipt: None,
            });
        }
        let sweep = self.0.as_mut().unwrap();
        if let Some((receipt, next)) = sweep.receipt.take() {
            if receipt.status() == EditCompletionStatus::Applied {
                sweep.cursor = next;
            }
        }
        let children = sweep.source.child_ids_slice(parent);
        let end = children.len().min(sweep.cursor.saturating_add(SCAN_TURN));
        let mut reads = Vec::with_capacity(32);
        reads.extend(guards);
        reads.push(NodeReadGuard::capture(snapshot, parent).ok_or("Sound Card route folder retired")?);
        let mut removals = Vec::with_capacity(DELETE_TURN);
        let mut next = sweep.cursor;
        for &id in &children[sweep.cursor..end] {
            if removals.len() == DELETE_TURN {
                break;
            }
            next += 1;
            let Some(node) = snapshot
                .node(id)
                .filter(|n| n.parent == Some(parent) && sweep.source.node(id).is_some_and(|old| old.uuid == n.uuid))
            else {
                continue;
            };
            let channel = super::super::find_child_by_key(snapshot, id, "channel");
            let valid = channel
                .and_then(|id| snapshot.node(id))
                .and_then(|n| n.param_value.as_ref())
                .is_some_and(|value| matches!(value, ParamValue::Reference(r) if sweep.desired.contains(&r.uuid())));
            if valid {
                continue;
            }
            reads.push(NodeReadGuard::capture(snapshot, node.id).ok_or("Sound Card route retired")?);
            if let Some(channel) = channel {
                reads.push(NodeReadGuard::capture(snapshot, channel).ok_or("Sound Card route channel retired")?);
            }
            removals.push(id);
        }
        if !removals.is_empty() {
            let mut edits = EditQueue::new();
            edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(reads, vec![lifetime])
                    .expect("eight prune targets and bounded intent reads"),
            });
            for node in removals {
                edits.push(Edit::RemoveNode { node });
            }
            sweep.receipt = Some((edits.completion(), next));
            ctx.edits.pending.extend(edits.drain());
            return Ok(true);
        }
        sweep.cursor = next;
        if next == children.len() {
            let stable = snapshot.subtree_revision(parent) == sweep.source.subtree_revision(parent);
            self.0 = None;
            // Own deletions also invalidate the source. A final fresh sweep accounts for
            // concurrent additions/rewires without restarting after every committed prefix.
            return Ok(!stable && !snapshot.child_ids_slice(parent).is_empty());
        }
        Ok(true)
    }
}

impl SoundCardModule {
    pub(in super::super) fn prune_routes(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &Arc<ProcessTreeSnapshot>,
        input: bool,
        desired: &[NodeReference],
    ) -> bool {
        if desired.len() > 256 {
            self.set_runtime_error(ctx, self.id(), "Sound Card prune channel capacity exceeded");
            return true;
        }
        let path = if input { INPUT_ROUTES_PATH } else { OUTPUT_ROUTES_PATH };
        let Some(parent) = find_path(snapshot, self.id(), path) else {
            if input {
                self.routes.prune_input = Pruning::default();
            } else {
                self.routes.prune_output = Pruning::default();
            }
            return false;
        };
        let mut guards = Vec::with_capacity(8);
        guards.push(identity(snapshot, self.id()).expect("live Sound Card module"));
        for path in [
            "connection/audio_driver",
            "connection/device",
            "connection/input_device",
            "connection/output_device",
            "connection/input_routing/channel_count",
            "connection/output_routing/channel_count",
        ] {
            if let Some(guard) =
                find_path(snapshot, self.id(), path).and_then(|id| NodeReadGuard::capture(snapshot, id))
            {
                guards.push(guard);
            }
        }
        let lifetime = self.routes.guard();
        let work = if input {
            &mut self.routes.prune_input
        } else {
            &mut self.routes.prune_output
        };
        match work.service(ctx, snapshot, parent, desired, guards, lifetime) {
            Ok(busy) => {
                if busy {
                    self.wake_route_work();
                }
                busy
            }
            Err(error) => {
                self.set_runtime_error(ctx, self.id(), &error);
                // Snapshot admission is bounded and retryable; retain the requested graph state.
                if let Some(wake) = &self.runtime_wake {
                    wake.wake_after(std::time::Duration::from_millis(250));
                }
                true
            }
        }
    }
}

#[cfg(test)]
mod tests;
