use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard},
    process_ctx::SubtreeRevision,
};
use golden_io::{PayloadPool, RetainedPayload, RetirementPool, RevisionSource, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::Duration,
};

struct ReadView {
    snapshot: Arc<ProcessTreeSnapshot>,
    // The canonical lease accounts for storage and owns final background destruction.
    _lease: RetainedPayload<ProcessTreeSnapshot>,
}

struct Normalize {
    dimension: SpatializerDimension,
    mode: SpatializerMode,
    sources: NodeId,
    targets: NodeId,
    counts: (usize, usize),
    index: usize,
    prepared: RetainedPayload<worker::Prepared>,
}

pub(super) struct Service {
    wake: Option<ModuleWake>,
    owner: RetirementPool,
    life: RevisionSource,
    generation: u64,
    normalize: Option<Normalize>,
    job: Option<worker::Job>,
    job_revision: Option<SubtreeRevision>,
    publication: Option<publication::Publication>,
    pending: Option<EditCompletion>,
    rejected: Option<SubtreeRevision>,
    settled: Option<SubtreeRevision>,
    read_view: Option<ReadView>,
    output_echoes_valid: bool,
    defaults: bool,
    warning: Option<String>,
    pub(super) reconcile: bool,
    #[cfg(test)]
    pub(super) control: Option<std::sync::Arc<worker::TestControl>>,
}

impl Service {
    pub fn new() -> Self {
        Self {
            wake: None,
            owner: RetirementPool::new(NonZeroUsize::MIN),
            life: RevisionSource::default(),
            generation: 0,
            normalize: None,
            job: None,
            job_revision: None,
            publication: None,
            pending: None,
            rejected: None,
            settled: None,
            read_view: None,
            output_echoes_valid: false,
            defaults: true,
            warning: None,
            reconcile: true,
            #[cfg(test)]
            control: None,
        }
    }
    pub fn register(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            self.cancel();
            self.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub fn request(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }
    pub fn invalidate_view(&mut self) {
        self.read_view = None;
    }
    pub fn invalidate_output_echoes(&mut self) {
        self.output_echoes_valid = false;
    }
    pub fn generated_output(&self, param: NodeId) -> bool {
        self.output_echoes_valid
            && self.publication.as_ref().is_some_and(|publication| {
                publication
                    .prepared
                    .output
                    .as_ref()
                    .is_some_and(|plan| plan.known_outputs.contains(&param))
            })
    }
    fn cache_view(&mut self, snapshot: Arc<ProcessTreeSnapshot>) -> Result<(), String> {
        if self
            .read_view
            .as_ref()
            .is_none_or(|view| !Arc::ptr_eq(&view.snapshot, &snapshot))
        {
            let lease = snapshot.try_lease().map_err(|error| error.to_string())?;
            self.read_view = Some(ReadView {
                snapshot,
                _lease: lease,
            });
        }
        Ok(())
    }
    pub fn observe_inbox(&mut self, ctx: &ProcessCtx) {
        if self.publication.is_some() && ctx.edits.pending.is_empty() {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                // Optional reuse only: capacity pressure falls back to the
                // ordinary fresh read on the next wake, without changing edits.
                let _ = self.cache_view(snapshot);
            }
        }
    }
    pub fn needs_tree_snapshot(&self) -> bool {
        // A receipt-only wake cannot plan work. Its completion callback wakes us
        // again after the applied prefix or rejected suffix has settled.
        self.read_view.is_none()
            && self
                .pending
                .as_ref()
                .is_none_or(|receipt| receipt.status() != EditCompletionStatus::Pending)
    }
    #[cfg(test)]
    pub(super) fn idle(&self) -> bool {
        self.pending.is_none()
            && (self.rejected.is_some()
                || self.job.is_none()
                    && self.normalize.is_none()
                    && self.publication.is_none()
                    && self.settled.is_some()
                    && !self.reconcile)
    }
    pub fn structure_changed(&mut self) {
        self.invalidate_view();
        self.rejected = None;
        self.reconcile = true;
        self.request();
    }
    #[cfg(test)]
    pub(super) fn worker_owner(&self) -> RetirementPool {
        self.owner.clone()
    }
    #[cfg(test)]
    pub(super) fn warning_text(&self) -> Option<&str> {
        self.warning.as_deref()
    }
    #[cfg(test)]
    pub(super) fn detached_nodes(&self) -> usize {
        self.publication
            .as_ref()
            .map_or(0, |publication| publication.detached_nodes())
    }
    #[cfg(test)]
    pub(super) fn staged_rows(&self) -> usize {
        self.publication
            .as_ref()
            .map_or(0, |publication| publication.staged_rows())
    }
    pub fn cancel(&mut self) {
        self.invalidate_view();
        self.generation = self.life.advance();
        self.normalize = None;
        self.job = None;
        self.job_revision = None;
        self.publication = None;
        self.settled = None;
        self.rejected = None;
        self.reconcile = true;
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        // A queued receipt retains its source until its applied prefix and
        // rejected suffix settle. Cancellation never treats it as replayable.
    }
    fn warning(&mut self, ctx: &mut ProcessCtx, module: NodeId, message: Option<String>) {
        if self.warning == message {
            return;
        }
        if let Some(message) = &message {
            NodeHandle::new(module).set_warning_with(ctx, Some("spatializer_service"), message, None);
        } else {
            NodeHandle::new(module).clear_warning(ctx, Some("spatializer_service"));
        }
        self.warning = message;
    }
    pub fn turn(&mut self, ctx: &mut ProcessCtx, module: NodeId, parameters: NodeId, values: NodeId) {
        if let Err(error) = self.inner(ctx, module, parameters, values) {
            self.invalidate_view();
            self.warning(ctx, module, Some(error));
            if let Some(wake) = &self.wake {
                wake.later(Duration::from_millis(250));
            }
        }
    }
    fn inner(
        &mut self,
        ctx: &mut ProcessCtx,
        module: NodeId,
        parameters: NodeId,
        values: NodeId,
    ) -> Result<(), String> {
        if let Some(receipt) = &self.pending {
            if receipt.status() == EditCompletionStatus::Pending {
                return Ok(());
            }
            if receipt.status() == EditCompletionStatus::Failed {
                self.normalize = None;
                self.publication = None;
                self.job_revision = None;
                self.settled = None;
                self.reconcile = true;
            }
            self.pending = None;
        }
        let Some(snapshot) = ctx
            .tree_snapshot_arc()
            .or_else(|| self.read_view.as_ref().map(|view| view.snapshot.clone()))
        else {
            return Ok(());
        };
        if snapshot.node(module).is_none_or(|node| !node.enabled) {
            self.invalidate_view();
            return Ok(());
        }
        let revision = snapshot
            .subtree_revision(parameters)
            .ok_or("Spatializer parameters are unavailable")?;
        if self.job_revision.is_some_and(|old| old != revision) {
            self.cancel();
        }
        if self.rejected == Some(revision) {
            self.invalidate_view();
            return Ok(());
        }
        self.rejected = None;
        if self.settled == Some(revision) && !self.reconcile {
            self.invalidate_view();
            return Ok(());
        }
        self.settled = None;
        let Some(sources) = snapshot.find_child_by_decl_id(parameters, "sources") else {
            return Ok(());
        };
        let Some(targets) = snapshot.find_child_by_decl_id(parameters, "targets") else {
            return Ok(());
        };
        let counts = (
            snapshot.child_ids_slice(sources).len(),
            snapshot.child_ids_slice(targets).len(),
        );
        let controls = worker::controls(&snapshot, parameters);
        let invalid = if snapshot.node(parameters).is_none_or(|node| node.parent != Some(module))
            || snapshot.node(values).is_none_or(|node| node.parent != Some(module))
        {
            Some("Spatializer Parameters and Values folders must belong to their module".to_string())
        } else if snapshot.child_ids_slice(parameters).len() > 32 {
            Some("Spatializer supports at most 32 direct parameter-folder children".to_string())
        } else {
            worker::admit(counts.0, counts.1)
                .err()
                .or_else(|| controls.as_ref().err().cloned())
        };
        if let Some(error) = invalid {
            self.cancel();
            self.rejected = Some(revision);
            self.warning(ctx, module, Some(error));
            return Ok(());
        }
        let (dimension, mode, _) = controls?;
        self.cache_view(snapshot.clone())?;
        if let Some(job) = &self.job {
            if job.running() {
                return Ok(());
            }
            let result = job.take().ok_or("Spatializer worker stopped without a result")?;
            self.job = None;
            match result {
                Ok(prepared) => {
                    if !prepared.repairs.is_empty() {
                        self.normalize = Some(Normalize {
                            dimension,
                            mode,
                            sources,
                            targets,
                            counts,
                            index: 0,
                            prepared,
                        });
                        self.job_revision = None;
                    } else if prepared.output.as_ref().map(|plan| plan.revision) == snapshot.subtree_revision(values) {
                        self.publication = Some(publication::Publication::new(prepared));
                        self.output_echoes_valid = true;
                        self.reconcile = false;
                    } else {
                        // Output changed while the worker was inspecting it. Recapture
                        // before applying a plan that deliberately omits unchanged cells.
                        self.job_revision = None;
                        self.request();
                        return Ok(());
                    }
                }
                Err(error) => {
                    self.job_revision = None;
                    self.normalize = None;
                    match error {
                        worker::Failure::Retry(message) => return Err(message.into()),
                        worker::Failure::Invalid(message) => {
                            self.cancel();
                            self.rejected = Some(revision);
                            self.warning(ctx, module, Some(message));
                            return Ok(());
                        }
                    }
                }
            }
        }
        if self.publication.is_some() {
            let publication = self.publication.as_mut().unwrap();
            if publication.done() {
                if self.reconcile {
                    self.publication = None;
                    self.job_revision = None;
                    self.reconcile = false;
                    self.request();
                    return Ok(());
                } else {
                    self.settled = Some(revision);
                    self.publication = None;
                    self.job_revision = None;
                    self.invalidate_view();
                    self.warning(ctx, module, None);
                    return Ok(());
                }
            }
            let mut edits = EditQueue::new();
            let mut reads = vec![publication.prepared.input_guard];
            let admission = edit_pool()?
                .try_reserve(EDIT_BYTES)
                .map_err(|_| "Spatializer publication lifetime capacity is full")?;
            publication.step(&snapshot, values, &mut edits, &mut reads)?;
            edits.retain_payload(publication.prepared.clone());
            if let Err(error) = self.submit(ctx, &snapshot, module, values, edits, reads, admission) {
                self.publication = None;
                self.job_revision = None;
                return Err(error);
            }
            if self.pending.is_none() {
                self.request();
            }
            return Ok(());
        }
        let mut edits = EditQueue::new();
        let reads = vec![NodeReadGuard::capture_subtree(&snapshot, parameters).unwrap()];
        let admission = edit_pool()?
            .try_reserve(EDIT_BYTES)
            .map_err(|_| "Spatializer structure lifetime capacity is full")?;
        if self.defaults {
            if counts.0 == 0 {
                edits.push(Edit::AddUserItemTree {
                    parent: sources,
                    tree: NodeTree::new(SpatializerSourceItem::new()),
                    prev_sibling: None,
                });
            }
            if counts.1 == 0 {
                edits.push(Edit::AddUserItemTree {
                    parent: targets,
                    tree: NodeTree::new(SpatializerTargetItem::new()),
                    prev_sibling: None,
                });
            }
            if !edits.pending.is_empty() {
                self.submit(ctx, &snapshot, module, values, edits, reads, admission)?;
                return Ok(());
            }
            self.defaults = false;
        }
        // Bounded repair of sparse persisted declarations precedes endpoint work.
        for tail in ["dimensions", "mode", "value_layout", "sources", "targets"] {
            for id in direct_child_ids_by_decl_tail(&snapshot, parameters, tail)
                .into_iter()
                .skip(1)
            {
                edits.push(Edit::RemoveNode { node: id });
            }
        }
        if !edits.pending.is_empty() {
            self.normalize = None;
            self.submit(ctx, &snapshot, module, values, edits, reads, admission)?;
            return Ok(());
        }
        if self.normalize.as_ref().is_some_and(|scan| {
            scan.dimension != dimension
                || scan.mode != mode
                || scan.sources != sources
                || scan.targets != targets
                || scan.counts != counts
        }) {
            self.normalize = None;
        }
        if let Some(scan) = self
            .normalize
            .as_mut()
            .filter(|scan| scan.index < scan.prepared.repairs.len())
        {
            let (id, source) = scan.prepared.repairs[scan.index];
            // Normalization itself changes the input revision. Recheck the
            // bounded endpoint shape before each live repair, including edits
            // authored after the worker captured this repair list.
            if snapshot.node(id).is_some_and(|item| {
                item.label.len() > worker::MAX_LABEL_BYTES
                    || snapshot.child_ids_slice(id).len() > worker::MAX_ENDPOINT_CHILDREN
            }) {
                self.cancel();
                self.rejected = Some(revision);
                self.warning(
                    ctx,
                    module,
                    Some("Spatializer endpoints support labels up to 512 bytes and at most 16 direct children".into()),
                );
                return Ok(());
            }
            if snapshot.node(id).is_some_and(|item| {
                item.parent == Some(if source { sources } else { targets })
                    && item.node_type
                        == if source {
                            SpatializerSourceItem::NODE_TYPE
                        } else {
                            SpatializerTargetItem::NODE_TYPE
                        }
            }) {
                let mut repair = ProcessCtx::new(ctx.phase, ctx.time);
                sync_position_child(&mut repair, &snapshot, id, dimension);
                sync_radius_child(
                    &mut repair,
                    &snapshot,
                    id,
                    if source {
                        mode.source_radius_required()
                    } else {
                        mode.target_radius_required()
                    },
                );
                if !source {
                    sync_freeze_radius_child(&mut repair, &snapshot, id, mode.target_freeze_radius_required());
                }
                edits.pending.extend(repair.edits.drain());
            }
            scan.index += 1;
            self.submit(ctx, &snapshot, module, values, edits, reads, admission)?;
            if self.pending.is_none() {
                self.request();
            }
            return Ok(());
        }
        let notifier = self
            .wake
            .as_ref()
            .ok_or("Spatializer worker wake is unavailable")?
            .notifier
            .clone();
        self.job = Some(worker::start(
            &snapshot,
            worker::Roots {
                parameters,
                sources,
                targets,
                values,
            },
            &self.owner,
            self.life.reader().guard_at(self.generation),
            notifier,
            #[cfg(test)]
            self.control.clone(),
        )?);
        self.normalize = None;
        self.job_revision = Some(revision);
        self.invalidate_view();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn submit(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        values: NodeId,
        mut edits: EditQueue,
        mut reads: Vec<NodeReadGuard>,
        admission: golden_io::PayloadReservation,
    ) -> Result<(), String> {
        if edits.pending.is_empty() {
            return Ok(());
        }
        reads.push(NodeReadGuard::capture_identity(snapshot, module).ok_or("Spatializer module disappeared")?);
        reads.push(NodeReadGuard::capture_structure(snapshot, values).ok_or("Spatializer values disappeared")?);
        if self.publication.is_some() && worker::trace_enabled() {
            eprintln!(
                "[spatializer] publication_turn nodes={} edits={} values={} trees={} reads={}",
                snapshot.node_count(),
                edits.pending.len(),
                edits
                    .pending
                    .iter()
                    .filter(|request| matches!(request.edit, Edit::SetParam { .. }))
                    .count(),
                edits
                    .pending
                    .iter()
                    .filter(|request| matches!(request.edit, Edit::AddNodeTree { .. }))
                    .count(),
                reads.len(),
            );
        }
        let preconditions = EditPreconditions::new(reads, vec![self.life.reader().guard_at(self.generation)])
            .ok_or("Spatializer publication read capacity exceeded")?;
        let mut guarded = EditQueue::new();
        guarded.push(Edit::CheckPreconditions { preconditions });
        guarded.pending.extend(edits.drain());
        let wake = WakeOnDrop(
            self.wake
                .as_ref()
                .ok_or("Spatializer worker wake is unavailable")?
                .notifier
                .clone(),
        );
        guarded.push(Edit::CallNodeMutation {
            node: module,
            needs_tree_snapshot: false,
            callback: Box::new(move |_, _| {
                drop(wake);
                Ok(())
            }),
        });
        self.pending = Some(guarded.completion());
        // The next planning turn must observe the applied prefix or a rejected suffix.
        self.invalidate_view();
        guarded.retain_payload(admission.retain((), EDIT_BYTES).unwrap());
        ctx.edits.pending.extend(guarded.drain());
        Ok(())
    }
}

const EDIT_BYTES: usize = 1024 * 1024;
fn edit_pool() -> Result<PayloadPool, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("spatializer-edits", 64, 64 * EDIT_BYTES).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
struct WakeOnDrop(WorkerNotifier);
impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        self.0.notify();
    }
}
