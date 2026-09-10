use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::Instant;

const DEFAULT_MAXIMUM_ITEMS: usize = 4_096;
const DEFAULT_MAXIMUM_WEIGHT: usize = 4 * 1024 * 1024;

struct PendingEntry<T> {
    value: T,
    weight: usize,
    admitted_at: Instant,
}

struct PendingState<T> {
    queue: Mutex<VecDeque<PendingEntry<T>>>,
    pending: AtomicBool,
    receiver_connected: AtomicBool,
    sender_count: AtomicUsize,
    maximum_items: usize,
    maximum_weight: usize,
    retained_weight: AtomicUsize,
    rejected: AtomicU64,
}

/// Sender half of a bounded event channel that exposes pending work without polling.
pub struct PendingSender<T> {
    state: Arc<PendingState<T>>,
}

/// Receiver half of a bounded event channel that exposes pending work without polling.
pub struct PendingReceiver<T> {
    state: Arc<PendingState<T>>,
}

/// Failure to admit an event without losing ownership of it.
#[derive(Debug, PartialEq, Eq)]
pub enum PendingSendError<T> {
    /// The receiver has been dropped.
    Disconnected(T),
    /// The configured item or retained-weight bound is exhausted.
    Full(T),
}

/// Lock-free and short-lock diagnostics for one pending channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingMetricsSnapshot {
    /// Current retained item count.
    pub depth: usize,
    /// Current caller-supplied retained weight.
    pub retained_weight: usize,
    /// Number of events rejected at capacity.
    pub rejected: u64,
    /// Age of the oldest retained event in nanoseconds.
    pub oldest_age_ns: u64,
    /// Whether a receiver drain turn is scheduled.
    pub ready: bool,
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

/// Creates a bounded worker channel with conservative defaults and a cheap event-ready signal.
pub fn pending_channel<T>() -> (PendingSender<T>, PendingReceiver<T>) {
    bounded_pending_channel(DEFAULT_MAXIMUM_ITEMS, DEFAULT_MAXIMUM_WEIGHT)
}

/// Creates a worker channel bounded by both item count and caller-supplied retained weight.
///
/// A successful send enqueues the item before publishing readiness. Consumers must use
/// [`PendingReceiver::drain_into`], which owns the clear-before-drain protocol and re-arms the
/// signal when a turn consumes its budget. Use [`PendingSender::send_weighted`] for dynamically
/// sized payloads; [`PendingSender::send`] assigns a weight of one.
pub fn bounded_pending_channel<T>(
    maximum_items: usize,
    maximum_weight: usize,
) -> (PendingSender<T>, PendingReceiver<T>) {
    assert!(maximum_items > 0, "pending channel must accept at least one item");
    assert!(maximum_weight > 0, "pending channel must accept positive weight");
    let state = Arc::new(PendingState {
        queue: Mutex::new(VecDeque::with_capacity(maximum_items)),
        pending: AtomicBool::new(false),
        receiver_connected: AtomicBool::new(true),
        sender_count: AtomicUsize::new(1),
        maximum_items,
        maximum_weight,
        retained_weight: AtomicUsize::new(0),
        rejected: AtomicU64::new(0),
    });
    (
        PendingSender {
            state: Arc::clone(&state),
        },
        PendingReceiver { state },
    )
}

impl<T> PendingSender<T> {
    /// Attempts to admit one fixed-weight event without blocking the producer.
    pub fn send(&self, value: T) -> Result<(), PendingSendError<T>> {
        self.send_weighted_before_publish(value, 1, || {})
    }

    /// Attempts to admit one dynamically sized event without blocking the producer.
    pub fn send_weighted(&self, value: T, weight: usize) -> Result<(), PendingSendError<T>> {
        self.send_weighted_before_publish(value, weight, || {})
    }

    #[cfg(test)]
    pub(crate) fn send_before_publish<F>(&self, value: T, before_publish: F) -> Result<(), PendingSendError<T>>
    where
        F: FnOnce(),
    {
        self.send_weighted_before_publish(value, 1, before_publish)
    }

    fn send_weighted_before_publish<F>(
        &self,
        value: T,
        weight: usize,
        before_publish: F,
    ) -> Result<(), PendingSendError<T>>
    where
        F: FnOnce(),
    {
        if !self.state.receiver_connected.load(Ordering::Acquire) {
            return Err(PendingSendError::Disconnected(value));
        }

        let mut queue = self.state.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !self.state.receiver_connected.load(Ordering::Acquire) {
            return Err(PendingSendError::Disconnected(value));
        }
        let retained_weight = self.state.retained_weight.load(Ordering::Relaxed);
        if queue.len() >= self.state.maximum_items
            || retained_weight
                .checked_add(weight)
                .is_none_or(|next| next > self.state.maximum_weight)
        {
            self.state.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(PendingSendError::Full(value));
        }
        queue.push_back(PendingEntry {
            value,
            weight,
            admitted_at: Instant::now(),
        });
        self.state
            .retained_weight
            .store(retained_weight + weight, Ordering::Relaxed);
        drop(queue);

        before_publish();
        self.state.pending.store(true, Ordering::Release);
        Ok(())
    }

    /// Captures current capacity and recovery diagnostics.
    pub fn metrics(&self) -> PendingMetricsSnapshot {
        metrics(&self.state)
    }
}

impl<T> Clone for PendingSender<T> {
    fn clone(&self) -> Self {
        self.state.sender_count.fetch_add(1, Ordering::Relaxed);
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> Drop for PendingSender<T> {
    fn drop(&mut self) {
        if self.state.sender_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.state.pending.store(true, Ordering::Release);
        }
    }
}

impl<T> PendingReceiver<T> {
    /// Reports whether a drain turn should be scheduled.
    pub fn has_pending(&self) -> bool {
        self.state.pending.load(Ordering::Acquire)
    }

    /// Clears readiness, drains at most `max_items`, and re-arms on budget exhaustion.
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
        self.state.pending.store(false, Ordering::Release);
        after_clear();
        let initial_len = output.len();
        let mut queue = self.state.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for _ in 0..max_items.get() {
            let Some(entry) = queue.pop_front() else {
                break;
            };
            self.state.retained_weight.fetch_sub(entry.weight, Ordering::Relaxed);
            output.push(entry.value);
        }

        let state = if !queue.is_empty() {
            self.state.pending.store(true, Ordering::Release);
            PendingDrainState::BudgetExhausted
        } else if self.state.sender_count.load(Ordering::Acquire) == 0 {
            PendingDrainState::Disconnected
        } else {
            PendingDrainState::Empty
        };
        PendingDrain {
            received: output.len() - initial_len,
            state,
        }
    }

    /// Captures current capacity and recovery diagnostics.
    pub fn metrics(&self) -> PendingMetricsSnapshot {
        metrics(&self.state)
    }
}

impl<T> Drop for PendingReceiver<T> {
    fn drop(&mut self) {
        self.state.receiver_connected.store(false, Ordering::Release);
        self.state
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.state.retained_weight.store(0, Ordering::Release);
    }
}

fn metrics<T>(state: &PendingState<T>) -> PendingMetricsSnapshot {
    let queue = state.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    PendingMetricsSnapshot {
        depth: queue.len(),
        retained_weight: state.retained_weight.load(Ordering::Relaxed),
        rejected: state.rejected.load(Ordering::Relaxed),
        oldest_age_ns: queue.front().map_or(0, |entry| {
            entry.admitted_at.elapsed().as_nanos().min(u64::MAX as u128) as u64
        }),
        ready: state.pending.load(Ordering::Acquire),
    }
}
