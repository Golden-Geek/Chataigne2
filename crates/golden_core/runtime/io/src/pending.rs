use std::num::NonZeroUsize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SendError, TryRecvError},
};

/// Sender half of an event channel that exposes pending work without polling.
pub struct PendingSender<T> {
    sender: mpsc::Sender<T>,
    pending: Arc<AtomicBool>,
}

/// Receiver half of an event channel that exposes pending work without polling.
pub struct PendingReceiver<T> {
    receiver: mpsc::Receiver<T>,
    pending: Arc<AtomicBool>,
}

/// Why a receiver-owned drain stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingDrainState {
    /// The queue was observed empty after all preceding accepted sends were drained.
    Empty,
    /// Every sender was dropped and no more items can arrive.
    Disconnected,
    /// The caller's service budget was consumed. Readiness is conservatively re-armed.
    BudgetExhausted,
}

/// Result of one bounded receiver-owned drain turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingDrain {
    pub received: usize,
    pub state: PendingDrainState,
}

/// Creates an unbounded worker channel with a cheap event-ready signal.
///
/// A successful send enqueues the item before publishing readiness. Consumers must use
/// [`PendingReceiver::drain_into`], which owns the clear-before-drain protocol and re-arms the
/// signal when a turn consumes its budget. Therefore every completed accepted send eventually
/// becomes observable, even when publication and draining overlap. A `send` error means the
/// receiver disconnected and does not publish false readiness.
pub fn pending_channel<T>() -> (PendingSender<T>, PendingReceiver<T>) {
    let pending = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    (
        PendingSender {
            sender,
            pending: Arc::clone(&pending),
        },
        PendingReceiver { receiver, pending },
    )
}

impl<T> PendingSender<T> {
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.send_before_publish(value, || {})
    }

    pub(crate) fn send_before_publish<F>(&self, value: T, before_publish: F) -> Result<(), SendError<T>>
    where
        F: FnOnce(),
    {
        self.sender.send(value)?;
        before_publish();
        self.pending.store(true, Ordering::Release);
        Ok(())
    }
}

impl<T> Clone for PendingSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            pending: Arc::clone(&self.pending),
        }
    }
}

impl<T> PendingReceiver<T> {
    /// Reports whether a drain turn should be scheduled.
    ///
    /// A `true` result can be conservative: a producer whose item was already drained may finish
    /// publication afterward. Callers must tolerate a drain that returns zero items.
    pub fn has_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    /// Clears readiness, drains at most `max_items`, and re-arms on budget exhaustion.
    ///
    /// `output` is appended to rather than cleared, allowing callers to reuse allocation. Empty
    /// and disconnected queues are reported separately. When the budget is exhausted readiness is
    /// always restored, even if the final item happened to empty the queue; the resulting extra
    /// scheduler pass is harmless and prevents stranded work.
    pub fn drain_into(&self, output: &mut Vec<T>, max_items: NonZeroUsize) -> PendingDrain {
        self.drain_into_after_clear(output, max_items, || {})
    }

    pub(crate) fn drain_into_after_clear<F>(
        &self,
        output: &mut Vec<T>,
        max_items: NonZeroUsize,
        after_clear: F,
    ) -> PendingDrain
    where
        F: FnOnce(),
    {
        self.pending.store(false, Ordering::Release);
        after_clear();
        let initial_len = output.len();

        for _ in 0..max_items.get() {
            match self.receiver.try_recv() {
                Ok(value) => output.push(value),
                Err(TryRecvError::Empty) => {
                    return PendingDrain {
                        received: output.len() - initial_len,
                        state: PendingDrainState::Empty,
                    };
                }
                Err(TryRecvError::Disconnected) => {
                    return PendingDrain {
                        received: output.len() - initial_len,
                        state: PendingDrainState::Disconnected,
                    };
                }
            }
        }

        self.pending.store(true, Ordering::Release);
        PendingDrain {
            received: output.len() - initial_len,
            state: PendingDrainState::BudgetExhausted,
        }
    }
}
