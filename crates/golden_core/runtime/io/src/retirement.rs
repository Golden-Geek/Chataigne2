use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Bounded retirement admission with a fixed number of active cleanup tasks.
///
/// The short state lock is held only while reserving or releasing a slot. Cleanup itself runs on a
/// dedicated named thread and never while holding the pool lock. A blocked cleanup therefore
/// consumes one explicit slot instead of causing an unbounded queue or thread cascade.
#[derive(Clone)]
pub struct RetirementPool {
    shared: Arc<RetirementShared>,
}

struct RetirementShared {
    capacity: usize,
    state: Mutex<RetirementState>,
    changed: Condvar,
}

#[derive(Default)]
struct RetirementState {
    active: usize,
    peak: usize,
    rejected: u64,
}

/// Current bounded-retirement occupancy and overload counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetirementMetricsSnapshot {
    pub capacity: usize,
    pub active: usize,
    pub peak: usize,
    pub rejected: u64,
}

/// Reservation guaranteeing that one retirement task may be started.
pub struct RetirementPermit {
    shared: Option<Arc<RetirementShared>>,
}

/// Recoverable retirement admission or worker-start failure retaining the original value.
pub struct RetirementError<T> {
    value: Box<T>,
    message: String,
}

impl RetirementPool {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            shared: Arc::new(RetirementShared {
                capacity: capacity.get(),
                state: Mutex::new(RetirementState::default()),
                changed: Condvar::new(),
            }),
        }
    }

    /// Reserves one retirement slot without starting cleanup or relinquishing resource ownership.
    pub fn try_reserve(&self) -> Result<RetirementPermit, RetirementCapacityError> {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active >= self.shared.capacity {
            state.rejected = state.rejected.saturating_add(1);
            return Err(RetirementCapacityError {
                capacity: self.shared.capacity,
            });
        }
        state.active += 1;
        state.peak = state.peak.max(state.active);
        drop(state);
        Ok(RetirementPermit {
            shared: Some(self.shared.clone()),
        })
    }

    /// Starts one bounded cleanup task or returns the value untouched on rejection.
    pub fn try_retire<T, F>(&self, name: impl Into<String>, value: T, retire: F) -> Result<(), RetirementError<T>>
    where
        T: Send + 'static,
        F: FnOnce(T) + Send + 'static,
    {
        let permit = match self.try_reserve() {
            Ok(permit) => permit,
            Err(error) => {
                return Err(RetirementError {
                    value: Box::new(value),
                    message: error.to_string(),
                });
            }
        };
        permit.spawn(name, value, retire)
    }

    pub fn metrics(&self) -> RetirementMetricsSnapshot {
        let state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        RetirementMetricsSnapshot {
            capacity: self.shared.capacity,
            active: state.active,
            peak: state.peak,
            rejected: state.rejected,
        }
    }

    /// Waits for all admitted cleanup tasks. Intended for orderly host shutdown and tests, never
    /// for engine ticks, render callbacks, or edit application.
    pub fn wait_for_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.active != 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, wait) = self
                .shared
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next;
            if wait.timed_out() && state.active != 0 {
                return false;
            }
        }
        true
    }
}

impl RetirementPermit {
    /// Starts cleanup using this guaranteed slot. Thread-start failure returns the value.
    pub fn spawn<T, F>(mut self, name: impl Into<String>, value: T, retire: F) -> Result<(), RetirementError<T>>
    where
        T: Send + 'static,
        F: FnOnce(T) + Send + 'static,
    {
        let value = Arc::new(Mutex::new(Some(value)));
        let worker_value = value.clone();
        let activity = RetirementActivity {
            shared: self
                .shared
                .take()
                .expect("retirement permit can start only one cleanup task"),
        };
        let result = thread::Builder::new().name(name.into()).spawn(move || {
            let _activity = activity;
            let value = worker_value
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
                .expect("retirement value is available exactly once");
            retire(value);
        });
        match result {
            Ok(_worker) => Ok(()),
            Err(error) => {
                let value = Arc::try_unwrap(value)
                    .ok()
                    .expect("failed retirement worker released its value owner")
                    .into_inner()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .expect("failed retirement worker preserved its value");
                Err(RetirementError {
                    value: Box::new(value),
                    message: format!("failed to start retirement worker: {error}"),
                })
            }
        }
    }
}

impl Drop for RetirementPermit {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.release();
        }
    }
}

struct RetirementActivity {
    shared: Arc<RetirementShared>,
}

impl Drop for RetirementActivity {
    fn drop(&mut self) {
        self.shared.release();
    }
}

impl RetirementShared {
    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(state.active > 0, "retirement capacity underflow");
        state.active -= 1;
        drop(state);
        self.changed.notify_all();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetirementCapacityError {
    capacity: usize,
}

impl fmt::Display for RetirementCapacityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "retirement capacity of {} active tasks is exhausted",
            self.capacity
        )
    }
}

impl std::error::Error for RetirementCapacityError {}

impl<T> RetirementError<T> {
    pub fn into_value(self) -> T {
        *self.value
    }
}

impl<T> fmt::Debug for RetirementError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetirementError")
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Display for RetirementError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl<T: Send + 'static> std::error::Error for RetirementError<T> {}
