use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// Single publication owner for a monotonically increasing native revision.
/// Dropping the owner invalidates outstanding guards, including revision zero.
#[derive(Debug, Default)]
pub struct RevisionSource(Arc<State>);

#[derive(Debug, Default)]
struct State {
    revision: AtomicU64,
    closed: AtomicBool,
}

/// Read-only native revision access without locking or retaining the payload.
#[derive(Clone, Debug)]
pub struct RevisionReader(Arc<State>);

/// Constant-time fence for one revision of one native source lifetime.
#[derive(Clone, Debug)]
pub struct RevisionGuard {
    reader: RevisionReader,
    expected: u64,
}

impl RevisionSource {
    /// Shares read access without transferring publication or retirement ownership.
    pub fn reader(&self) -> RevisionReader {
        RevisionReader(self.0.clone())
    }

    /// Advances before publishing the associated state. Exhaustion fails rather
    /// than wrapping and making an old guard current again.
    pub fn advance(&self) -> u64 {
        self.0
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| value.checked_add(1))
            .expect("native revision exhausted")
            + 1
    }
}

impl Drop for RevisionSource {
    fn drop(&mut self) {
        self.0.closed.store(true, Ordering::Release);
    }
}

impl RevisionReader {
    /// Last revision, including after retirement. Use a guard to check liveness.
    pub fn current(&self) -> u64 {
        self.0.revision.load(Ordering::Acquire)
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
    /// Conservative shared allocation charge. Each owner charges the full
    /// clock allocation rather than depending on the current Arc strong count.
    pub const fn retained_heap_bytes(&self) -> usize {
        size_of::<State>() + 2 * size_of::<usize>()
    }

    /// A concurrent publication may follow this check. Treat success as the
    /// admission boundary, not as a lock on future native state.
    pub fn is_current(&self) -> bool {
        !self.reader.0.closed.load(Ordering::Acquire) && self.reader.current() == self.expected
    }
}

#[cfg(test)]
mod tests;
