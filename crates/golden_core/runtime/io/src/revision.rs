use crate::WorkerNotifier;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// Single publication owner for a monotonically increasing native revision.
/// Dropping the owner invalidates outstanding guards, including revision zero.
#[derive(Debug, Default)]
pub struct RevisionSource {
    state: Arc<State>,
    registry: Arc<Registry>,
}

#[derive(Debug, Default)]
struct State {
    revision: AtomicU64,
    closed: AtomicBool,
}

#[derive(Debug, Default)]
struct Registry {
    watchers: Mutex<Vec<Weak<Watch>>>,
}

#[derive(Debug)]
struct Watch {
    expected: u64,
    notifier: WorkerNotifier,
}

/// Keeps a one-shot invalidation notification registered. Dropping it cancels the watch.
#[derive(Debug)]
pub struct RevisionSubscription {
    _watch: Arc<Watch>,
}

impl Registry {
    fn notify_invalidated(&self, state: &State) {
        let mut notifications = Vec::new();
        {
            let mut watchers = self.watchers.lock().expect("revision watchers lock poisoned");
            let current = state.revision.load(Ordering::Acquire);
            let closed = state.closed.load(Ordering::Acquire);
            watchers.retain(|watch| {
                let Some(watch) = watch.upgrade() else { return false };
                if closed || watch.expected != current {
                    notifications.push(watch);
                    false
                } else {
                    true
                }
            });
        }
        // A notifier may register another watch; never invoke it while holding the registry lock.
        for watch in notifications {
            watch.notifier.notify();
        }
    }
}

/// Read-only native revision access without locking or retaining the payload.
#[derive(Clone, Debug)]
pub struct RevisionReader {
    state: Arc<State>,
    registry: Weak<Registry>,
}

/// Constant-time fence for one revision of one native source lifetime.
#[derive(Clone, Debug)]
pub struct RevisionGuard {
    reader: RevisionReader,
    expected: u64,
}

impl RevisionSource {
    /// Shares read access without transferring publication or retirement ownership.
    pub fn reader(&self) -> RevisionReader {
        RevisionReader {
            state: self.state.clone(),
            registry: Arc::downgrade(&self.registry),
        }
    }

    /// Advances before publishing the associated state. Exhaustion fails rather
    /// than wrapping and making an old guard current again.
    pub fn advance(&self) -> u64 {
        let revision = self
            .state
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| value.checked_add(1))
            .expect("native revision exhausted")
            + 1;
        self.registry.notify_invalidated(&self.state);
        revision
    }
}

impl Drop for RevisionSource {
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Release);
        self.registry.notify_invalidated(&self.state);
    }
}

impl RevisionReader {
    /// Last revision, including after retirement. Use a guard to check liveness.
    pub fn current(&self) -> u64 {
        self.state.revision.load(Ordering::Acquire)
    }

    /// Guards a revision captured with an earlier immutable publication. This
    /// deliberately does not substitute the latest revision for the captured one.
    pub fn guard_at(&self, expected: u64) -> RevisionGuard {
        RevisionGuard {
            reader: self.clone(),
            expected,
        }
    }
}

impl RevisionGuard {
    /// Wakes a worker once this captured revision becomes stale, including source retirement.
    /// An already-stale guard notifies immediately. The callback must remain nonblocking.
    pub fn subscribe(&self, notifier: WorkerNotifier) -> RevisionSubscription {
        let watch = Arc::new(Watch {
            expected: self.expected,
            notifier,
        });
        let stale = if let Some(registry) = self.reader.registry.upgrade() {
            let mut watchers = registry.watchers.lock().expect("revision watchers lock poisoned");
            if self.is_current() {
                watchers.retain(|watch| watch.strong_count() > 0);
                watchers.push(Arc::downgrade(&watch));
                false
            } else {
                true
            }
        } else {
            true
        };
        if stale {
            watch.notifier.notify();
        }
        RevisionSubscription { _watch: watch }
    }

    /// Fixed allocations retained by this guard, including the Registry Arc allocation
    /// retained by its Weak handle. Only RevisionSource owns the registry's dynamic storage:
    /// dropping it frees the watcher Vec even while guards/subscriptions remain alive.
    /// Subscriptions own notifier callbacks and their captured state outside plan payloads.
    pub const fn retained_heap_bytes(&self) -> usize {
        size_of::<State>() + size_of::<Registry>() + 4 * size_of::<usize>()
    }

    /// A concurrent publication may follow this check. Treat success as the
    /// admission boundary, not as a lock on future native state.
    pub fn is_current(&self) -> bool {
        !self.reader.state.closed.load(Ordering::Acquire) && self.reader.current() == self.expected
    }
}

#[cfg(test)]
mod tests;
