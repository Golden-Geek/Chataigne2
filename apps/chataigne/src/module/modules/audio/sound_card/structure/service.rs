//! One retained pass, bounded inspection/publication, and one graph receipt.
use super::*;
use golden_core::edit::{EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard};
use golden_io::{RetainedPayload, RevisionSource};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const SCAN_TURN: usize = 32;
pub(super) const WRITE_TURN: usize = 8;
pub(super) const NAME_BYTES: usize = 64 * 1024;

#[derive(Default)]
pub(in super::super) struct DerivedWork {
    lifetime: RevisionSource,
    pass: Option<Pass>,
    receipt: Option<(EditCompletion, Position)>,
    retry_at: Option<Instant>,
    restart_pending: bool,
    rescan_requested: bool,
}

struct Pass {
    source: RetainedPayload<ProcessTreeSnapshot>,
    position: Position,
    wanted: [std::collections::HashSet<NodeUuid>; 4],
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Stage {
    #[default]
    InputRoot,
    OutputRoot,
    InputGains,
    OutputGains,
    StaleInputGains,
    StaleOutputGains,
    InputLevels,
    OutputLevels,
    StaleInputLevels,
    StaleOutputLevels,
    SpectralParameters,
    PitchValues,
    SpectralValues,
    ClearSpectralValues,
    ObsoleteValues,
    EmptyParameters,
    DirectionOrder,
    Done,
}

impl Stage {
    pub(super) fn next(self) -> Self {
        match self {
            Self::InputRoot => Self::OutputRoot,
            Self::OutputRoot => Self::InputGains,
            Self::InputGains => Self::OutputGains,
            Self::OutputGains => Self::StaleInputGains,
            Self::StaleInputGains => Self::StaleOutputGains,
            Self::StaleOutputGains => Self::InputLevels,
            Self::InputLevels => Self::OutputLevels,
            Self::OutputLevels => Self::StaleInputLevels,
            Self::StaleInputLevels => Self::StaleOutputLevels,
            Self::StaleOutputLevels => Self::SpectralParameters,
            Self::SpectralParameters => Self::PitchValues,
            Self::PitchValues => Self::SpectralValues,
            Self::SpectralValues => Self::ClearSpectralValues,
            Self::ClearSpectralValues => Self::ObsoleteValues,
            Self::ObsoleteValues => Self::EmptyParameters,
            Self::EmptyParameters => Self::DirectionOrder,
            Self::DirectionOrder => Self::Done,
            Self::Done => Self::Done,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Position {
    pub stage: Stage,
    pub row: usize,
    pub retained: Option<NodeId>,
}

pub(super) struct Repair {
    pub edits: Vec<Edit>,
    pub reads: Vec<NodeId>,
    /// Revisit this row after removing part of a legacy container's children.
    pub revisit: bool,
}

impl Repair {
    pub(super) fn new(edits: Vec<Edit>, reads: Vec<NodeId>) -> Self {
        Self {
            edits,
            reads,
            revisit: false,
        }
    }
}

impl DerivedWork {
    pub(in super::super) fn invalidate(&mut self) {
        self.lifetime.advance();
        self.pass = None;
        self.receipt = None;
        self.retry_at = None;
        self.restart_pending = false;
        self.rescan_requested = false;
    }

    pub(in super::super) fn request(&mut self) {
        self.rescan_requested = true;
    }

    pub(in super::super) fn busy(&self) -> bool {
        self.pass.is_some()
            || self.receipt.is_some()
            || self.retry_at.is_some()
            || self.restart_pending
            || self.rescan_requested
    }

    pub(in super::super) fn waiting(&self) -> bool {
        self.retry_at.is_some_and(|at| Instant::now() < at)
            || self
                .receipt
                .as_ref()
                .is_some_and(|(r, _)| r.status() == EditCompletionStatus::Pending)
    }

    pub(in super::super) fn needs_snapshot(&self) -> bool {
        self.busy() && !self.waiting()
    }

    pub(in super::super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
    ) -> Result<bool, String> {
        let result = self.service_inner(ctx, snapshot, module);
        if result.is_err() {
            self.pass = None;
            self.retry_at = Some(Instant::now() + Duration::from_millis(250));
        }
        result
    }

    fn service_inner(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
    ) -> Result<bool, String> {
        if self.waiting() {
            return Ok(true);
        }
        self.retry_at = None;
        if let Some((receipt, position)) = self.receipt.take() {
            if receipt.status() != EditCompletionStatus::Applied {
                // Re-read the entire failed prefix. Already committed repairs are idempotent.
                if let Some(pass) = &mut self.pass {
                    pass.position = position;
                }
            }
        }
        if self.pass.as_ref().is_some_and(|pass| {
            super::INTENT_PATHS.iter().any(|path| {
                let old = find_path(&pass.source, module, path);
                let new = find_path(snapshot, module, path);
                old != new || old.is_some_and(|id| pass.source.subtree_revision(id) != snapshot.subtree_revision(id))
            })
        }) {
            self.pass = None;
        }
        if self.pass.is_none() {
            let source = snapshot.try_lease().map_err(|_| {
                self.retry_at = Some(Instant::now() + Duration::from_millis(250));
                "Sound Card structure snapshot capacity exhausted"
            })?;
            self.pass = Some(Pass {
                source,
                position: Position::default(),
                wanted: Default::default(),
            });
            self.restart_pending = false;
            self.rescan_requested = false;
        }
        let pass = self.pass.as_mut().unwrap();
        if snapshot.node(module).is_none()
            || pass.source.node(module).map(|n| n.uuid) != snapshot.node(module).map(|n| n.uuid)
        {
            self.invalidate();
            return Ok(false);
        }
        let before = pass.position;
        let mut edits = EditQueue::new();
        let mut read_ids = Vec::with_capacity(32);
        for path in super::INTENT_PATHS {
            if let Some(id) = find_path(snapshot, module, path) {
                read_ids.push(id);
            }
        }
        let mut writes = 0;
        let mut done = false;
        for _ in 0..SCAN_TURN {
            let saved = pass.position;
            let Some(repair) = super::step(&pass.source, snapshot, module, &mut pass.position, &mut pass.wanted)?
            else {
                if pass.position.stage == Stage::Done {
                    done = true;
                    break;
                }
                continue;
            };
            if writes + repair.edits.len() > WRITE_TURN {
                pass.position = saved;
                break;
            }
            let additional = repair.reads.iter().filter(|id| !read_ids.contains(id)).count();
            if read_ids.len() + additional + 1 > 32 {
                pass.position = saved;
                break;
            }
            for id in repair.reads {
                if !read_ids.contains(&id) {
                    read_ids.push(id);
                }
            }
            let removes_container = repair.edits.iter().any(|edit| {
                matches!(edit, Edit::RemoveNode { node } if snapshot.node(*node).is_some_and(|n| n.child_count != 0))
            });
            writes += repair.edits.len();
            for edit in repair.edits {
                edits.push(edit);
            }
            // Later stages must not queue edits against descendants of a root
            // that this prefix retires. Re-read the graph after its receipt.
            if repair.revisit || removes_container || writes == WRITE_TURN {
                break;
            }
        }
        if writes != 0 {
            let mut reads = Vec::with_capacity(32);
            reads.push(NodeReadGuard::capture_identity(snapshot, module).ok_or("Sound Card retired")?);
            for id in read_ids {
                reads.push(NodeReadGuard::capture(snapshot, id).ok_or("Sound Card structure target retired")?);
            }
            let reader = self.lifetime.reader();
            let mut fenced = EditQueue::new();
            // Fence once: earlier edits in our own prefix change parent revisions.
            fenced.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(reads, vec![reader.guard_at(reader.current())])
                    .expect("bounded structure intent and target reads"),
            });
            fenced.pending.extend(edits.drain());
            self.receipt = Some((fenced.completion(), before));
            self.rescan_requested = true;
            ctx.edits.pending.extend(fenced.drain());
            return Ok(true);
        }
        if done {
            // Authored structure/meta callbacks request a fresh pass; our own writes do
            // too. Meter/value-only changes cannot perpetually invalidate a long scan.
            let stable = !self.rescan_requested;
            self.pass = None;
            self.restart_pending = !stable;
            return Ok(!stable);
        }
        Ok(true)
    }
}
