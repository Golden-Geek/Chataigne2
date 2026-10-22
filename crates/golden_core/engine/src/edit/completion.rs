use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Disposition of the requests captured by an [`EditCompletion`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCompletionStatus {
    /// At least one request is still queued, deferred, or inside an atomic operation.
    Pending,
    /// Every captured request completed and any enclosing atomic operation committed.
    Applied,
    /// At least one request was rejected, discarded, superseded, or rolled back.
    /// An applied prefix may remain; this is not permission to replay the batch.
    Failed,
}

/// Read-only completion of a particular set of deferred edits, independent of snapshots.
///
/// This observes request application, not subsequent event delivery, external side effects,
/// or edits later emitted by event subscribers. Ordinary receipts retain no payloads;
/// queues may separately attach an admitted source lease through `retain_payload`.
#[derive(Clone)]
pub struct EditCompletion(Arc<CompletionState>);

struct CompletionState {
    pending: AtomicUsize,
    failed: AtomicBool,
    _retained: Option<Box<dyn Send + Sync>>,
}

impl EditCompletion {
    pub(super) fn new(requests: usize) -> Self {
        Self(Arc::new(CompletionState {
            pending: AtomicUsize::new(requests),
            failed: AtomicBool::new(false),
            _retained: None,
        }))
    }

    pub(super) fn retaining<T: Send + Sync + 'static>(requests: usize, payload: golden_io::RetainedPayload<T>) -> Self {
        Self(Arc::new(CompletionState {
            pending: AtomicUsize::new(requests),
            failed: AtomicBool::new(false),
            _retained: Some(Box::new(payload)),
        }))
    }

    /// Returns a terminal status only after every captured request has settled.
    pub fn status(&self) -> EditCompletionStatus {
        if self.0.pending.load(Ordering::Acquire) != 0 {
            EditCompletionStatus::Pending
        } else if self.0.failed.load(Ordering::Acquire) {
            EditCompletionStatus::Failed
        } else {
            EditCompletionStatus::Applied
        }
    }

    pub(super) fn token(&self) -> CompletionToken {
        CompletionToken {
            state: Arc::clone(&self.0),
            applied: false,
        }
    }
}

/// Dropping an unfinished token also covers discarded queue suffixes and engine teardown.
pub(crate) struct CompletionToken {
    state: Arc<CompletionState>,
    applied: bool,
}

impl CompletionToken {
    pub(crate) fn applied(mut self) {
        self.applied = true;
    }
}

impl Drop for CompletionToken {
    fn drop(&mut self) {
        if !self.applied {
            self.state.failed.store(true, Ordering::Release);
        }
        self.state.pending.fetch_sub(1, Ordering::AcqRel);
    }
}
