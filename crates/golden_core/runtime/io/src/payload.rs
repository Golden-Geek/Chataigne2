use std::{
    any::Any,
    io,
    ops::Deref,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

type ErasedPayload = Arc<dyn Any + Send + Sync>;
type ClaimedPayload = Box<dyn Any + Send + Sync>;
type Claim = fn(ErasedPayload) -> Result<ClaimedPayload, ErasedPayload>;

struct Entry {
    value: ErasedPayload,
    claim: Claim,
    weight: usize,
    external: bool,
}

// A strong-count observation is only a scheduling hint: Weak::upgrade can race it.
// Claim under the registry lock so concurrent registration cannot miss an entry
// being claimed. The returned value is destroyed only after releasing that lock.
fn claim<T: Send + Sync + 'static>(value: ErasedPayload) -> Result<ClaimedPayload, ErasedPayload> {
    let Ok(value) = value.downcast::<T>() else {
        unreachable!("payload disposer type matches its entry");
    };
    match Arc::try_unwrap(value) {
        Ok(value) => Ok(Box::new(value)),
        Err(value) => Err(value),
    }
}

struct State {
    entries: Vec<Entry>,
    owners: usize,
    active: usize,
    retained_bytes: usize,
    external: usize,
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

/// Capacity admitted before constructing a payload. Dropping an unused reservation
/// releases it; sealing transfers it to the background disposal lifetime.
pub struct PayloadReservation {
    pool: Option<PayloadPool>,
    weight: usize,
}

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
                external: 0,
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
        if !self.0.admit(&mut state, weight) {
            return Err(PayloadCapacityError(value));
        }
        let value = Arc::new(value);
        state.entries.push(Entry {
            value: value.clone(),
            claim: claim::<T>,
            weight,
            external: false,
        });
        self.0.changed.notify_all();
        Ok(RetainedPayload {
            value: Some(value),
            shared: self.0.clone(),
        })
    }

    /// Reserve before materializing data on a consumer thread. The eventual retained
    /// weight must fit this reservation. Reserved items also keep the disposer alive.
    pub fn try_reserve(&self, weight: usize) -> Result<PayloadReservation, PayloadCapacityError<()>> {
        let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
        if !self.0.admit(&mut state, weight) {
            return Err(PayloadCapacityError(()));
        }
        state.owners += 1;
        Ok(PayloadReservation {
            pool: Some(Self(self.0.clone())),
            weight,
        })
    }

    /// Retains an existing immutable Arc, including clones held outside this API.
    /// Use one canonical pool for a given Arc; independent final owners in separate pools
    /// would keep each other alive. Repeated registration in this pool shares one reservation.
    /// External Arc releases have no notification, so the disposer checks these entries at
    /// most once per 100ms while idle. No polling or final destruction runs on consumers.
    pub fn try_retain_arc<T: Send + Sync + 'static>(
        &self,
        value: Arc<T>,
        weight: usize,
    ) -> Result<RetainedPayload<T>, PayloadCapacityError<Arc<T>>> {
        let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
        let pointer = Arc::as_ptr(&value).cast::<()>();
        if let Some(entry) = state
            .entries
            .iter()
            .find(|entry| Arc::as_ptr(&entry.value).cast::<()>() == pointer)
        {
            if !entry.external || weight > entry.weight {
                return Err(PayloadCapacityError(value));
            }
        } else {
            if !self.0.admit(&mut state, weight) {
                return Err(PayloadCapacityError(value));
            }
            state.entries.push(Entry {
                value: value.clone(),
                claim: claim::<T>,
                weight,
                external: true,
            });
            state.external += 1;
            self.0.changed.notify_all();
        }
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

impl PayloadReservation {
    /// Seal admitted capacity without copying the value. Unused bytes become available
    /// immediately; item and used-byte capacity stay charged until destruction finishes.
    /// Oversized values are returned to their producer and the reservation is released.
    pub fn retain<T: Send + Sync + 'static>(
        self,
        value: T,
        weight: usize,
    ) -> Result<RetainedPayload<T>, PayloadCapacityError<T>> {
        let (value, shared) = self.seal(value, weight, false)?;
        Ok(RetainedPayload {
            value: Some(value),
            shared,
        })
    }

    /// Seal a fresh immutable Arc for APIs that already route shared data. The pool
    /// remains its canonical final owner even after all handles disappear. Raw Arc
    /// releases cannot notify the worker, so only its disposer polls these entries
    /// at most once per 100ms. Never register the Arc in an independent pool.
    pub fn share<T: Send + Sync + 'static>(self, value: T, weight: usize) -> Result<Arc<T>, PayloadCapacityError<T>> {
        self.seal(value, weight, true).map(|(value, _)| value)
    }

    fn seal<T: Send + Sync + 'static>(
        mut self,
        value: T,
        weight: usize,
        external: bool,
    ) -> Result<(Arc<T>, Arc<Shared>), PayloadCapacityError<T>> {
        if weight > self.weight {
            return Err(PayloadCapacityError(value));
        }
        let pool = self.pool.take().expect("live payload reservation");
        let value = Arc::new(value);
        {
            let mut state = pool.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.retained_bytes -= self.weight - weight;
            state.external += usize::from(external);
            state.entries.push(Entry {
                value: value.clone(),
                claim: claim::<T>,
                weight,
                external,
            });
        }
        pool.0.changed.notify_all();
        Ok((value, pool.0.clone()))
    }
}

impl Drop for PayloadReservation {
    fn drop(&mut self) {
        if let Some(pool) = &self.pool {
            let mut state = pool.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.active -= 1;
            state.retained_bytes -= self.weight;
            pool.0.changed.notify_all();
        }
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
    fn admit(&self, state: &mut State, weight: usize) -> bool {
        if state.active == self.maximum_items
            || state
                .retained_bytes
                .checked_add(weight)
                .is_none_or(|bytes| bytes > self.maximum_bytes)
        {
            return false;
        }
        state.active += 1;
        state.retained_bytes += weight;
        true
    }

    fn run(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(index) = state
                .entries
                .iter()
                .position(|entry| Arc::strong_count(&entry.value) == 1)
            {
                let entry = state.entries.swap_remove(index);
                let value = match (entry.claim)(entry.value) {
                    Ok(value) => value,
                    Err(value) => {
                        state.entries.push(Entry { value, ..entry });
                        continue;
                    }
                };
                drop(state);
                // No payload destructor or arbitrary native cleanup runs under the state lock.
                let _ = catch_unwind(AssertUnwindSafe(|| drop(value)));
                state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                state.active -= 1;
                state.external -= usize::from(entry.external);
                state.retained_bytes -= entry.weight;
                self.changed.notify_all();
            } else if state.owners == 0 && state.active == 0 {
                return;
            } else {
                state = if state.external > 0 {
                    self.changed
                        .wait_timeout(state, Duration::from_millis(100))
                        .unwrap_or_else(|p| p.into_inner())
                        .0
                } else {
                    self.changed.wait(state).unwrap_or_else(|p| p.into_inner())
                };
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
