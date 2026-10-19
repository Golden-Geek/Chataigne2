use std::{
    any::Any,
    io,
    ops::Deref,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

struct Entry {
    value: Arc<dyn Any + Send + Sync>,
    weight: usize,
}

struct State {
    entries: Vec<Entry>,
    owners: usize,
    active: usize,
    retained_bytes: usize,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    maximum_items: usize,
    maximum_bytes: usize,
}

/// A bounded payload lifetime pool with one background disposer.
/// Admission includes consumer-owned and actively dropping payloads. A blocked destructor
/// keeps its reservation; it cannot trigger another cleanup thread or release capacity early.
pub struct PayloadPool(Arc<Shared>);

/// Immutable shared data whose final destructor always runs on its pool's worker.
pub struct RetainedPayload<T: Send + Sync + 'static> {
    value: Option<Arc<T>>,
    shared: Arc<Shared>,
}

/// Rejected admission retains the original prepared value for its producer.
pub struct PayloadCapacityError<T>(T);

impl PayloadPool {
    pub fn new(name: impl Into<String>, maximum_items: usize, maximum_bytes: usize) -> io::Result<Self> {
        assert!(maximum_items > 0 && maximum_bytes > 0);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: Vec::new(),
                owners: 1,
                active: 0,
                retained_bytes: 0,
            }),
            changed: Condvar::new(),
            maximum_items,
            maximum_bytes,
        });
        let worker = shared.clone();
        thread::Builder::new().name(name.into()).spawn(move || worker.run())?;
        Ok(Self(shared))
    }

    /// Call on the preparing worker. Weight must cover the value and its retained heap storage
    /// throughout its lifetime; do not grow interior allocations beyond that reservation.
    /// Failure does not transfer ownership or perform cleanup on a consumer's behalf.
    pub fn try_retain<T: Send + Sync + 'static>(
        &self,
        value: T,
        weight: usize,
    ) -> Result<RetainedPayload<T>, PayloadCapacityError<T>> {
        let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.active == self.0.maximum_items
            || state
                .retained_bytes
                .checked_add(weight)
                .is_none_or(|bytes| bytes > self.0.maximum_bytes)
        {
            return Err(PayloadCapacityError(value));
        }
        let value = Arc::new(value);
        state.entries.push(Entry {
            value: value.clone(),
            weight,
        });
        state.active += 1;
        state.retained_bytes += weight;
        self.0.changed.notify_all();
        Ok(RetainedPayload {
            value: Some(value),
            shared: self.0.clone(),
        })
    }

    /// Intended for host shutdown and controlled tests, never a graph callback.
    pub fn wait_for_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
        while state.active > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            state = self
                .0
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        true
    }
}

impl Clone for PayloadPool {
    fn clone(&self) -> Self {
        self.0.state.lock().unwrap_or_else(|p| p.into_inner()).owners += 1;
        Self(self.0.clone())
    }
}

impl Drop for PayloadPool {
    fn drop(&mut self) {
        self.0.state.lock().unwrap_or_else(|p| p.into_inner()).owners -= 1;
        self.0.changed.notify_all();
    }
}

impl Shared {
    fn run(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(index) = state
                .entries
                .iter()
                .position(|entry| Arc::strong_count(&entry.value) == 1)
            {
                let entry = state.entries.swap_remove(index);
                drop(state);
                // No payload destructor or arbitrary native cleanup runs under the state lock.
                let _ = catch_unwind(AssertUnwindSafe(|| drop(entry.value)));
                state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                state.active -= 1;
                state.retained_bytes -= entry.weight;
                self.changed.notify_all();
            } else if state.owners == 0 && state.active == 0 {
                return;
            } else {
                state = self.changed.wait(state).unwrap_or_else(|p| p.into_inner());
            }
        }
    }
}

impl<T: Send + Sync + 'static> Deref for RetainedPayload<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value.as_deref().expect("live retained payload")
    }
}

impl<T: Send + Sync + 'static> Clone for RetainedPayload<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            shared: self.shared.clone(),
        }
    }
}

impl<T: Send + Sync + 'static> Drop for RetainedPayload<T> {
    fn drop(&mut self) {
        // Pair the last consumer release with the collector's condition-variable check.
        let _state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        drop(self.value.take());
        self.shared.changed.notify_all();
    }
}

impl<T> PayloadCapacityError<T> {
    pub fn into_value(self) -> T {
        self.0
    }
}

impl<T> std::fmt::Debug for PayloadCapacityError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("payload capacity exhausted")
    }
}

impl<T> std::fmt::Display for PayloadCapacityError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("payload lifetime capacity exhausted")
    }
}

impl<T: Send + 'static> std::error::Error for PayloadCapacityError<T> {}

impl<T: Send + Sync + 'static> std::fmt::Debug for RetainedPayload<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedPayload").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
