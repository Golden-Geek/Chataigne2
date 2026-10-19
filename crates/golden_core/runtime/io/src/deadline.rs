use std::{
    sync::{Arc, Condvar, Mutex, OnceLock, Weak},
    thread,
    time::{Duration, Instant},
};

use crate::WorkerNotifier;

const DELAYED_WAKE_CAPACITY: usize = 1024;

struct TimerState {
    registrations: Vec<Weak<DelayedNotifierInner>>,
}

struct WakeTimer {
    state: Mutex<TimerState>,
    changed: Condvar,
}

struct DelayedNotifierInner {
    notifier: WorkerNotifier,
    deadline: Mutex<Option<Instant>>,
    timer: Arc<WakeTimer>,
}

/// One coalesced deadline on a shared, bounded host timer.
/// Payloads stay at their owning boundary; the timer only publishes readiness.
#[derive(Clone)]
pub struct DelayedNotifier(Arc<DelayedNotifierInner>);

impl DelayedNotifier {
    pub fn new(notifier: WorkerNotifier) -> Result<Self, String> {
        wake_timer()?.register(notifier)
    }

    pub fn wake(&self) {
        self.0.notifier.notify();
    }

    pub fn wake_after(&self, delay: Duration) {
        // Pair deadline publication with the timer's condition-variable lock. One source
        // retains only its earliest wake; the runtime still owns the actual retry deadline.
        let _state = self.0.timer.state.lock().unwrap_or_else(|p| p.into_inner());
        let mut deadline = self.0.deadline.lock().unwrap_or_else(|p| p.into_inner());
        let requested = Instant::now() + delay;
        *deadline = Some(deadline.map_or(requested, |old| old.min(requested)));
        self.0.timer.changed.notify_one();
    }
    /// Cancels a deadline without disconnecting this reusable notification source.
    pub fn cancel_deadline(&self) {
        let _state = self.0.timer.state.lock().unwrap_or_else(|p| p.into_inner());
        *self.0.deadline.lock().unwrap_or_else(|p| p.into_inner()) = None;
        self.0.timer.changed.notify_one();
    }
}

impl WakeTimer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(TimerState {
                registrations: Vec::new(),
            }),
            changed: Condvar::new(),
        })
    }

    fn register(self: &Arc<Self>, notifier: WorkerNotifier) -> Result<DelayedNotifier, String> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.registrations.retain(|entry| entry.strong_count() != 0);
        if state.registrations.len() == DELAYED_WAKE_CAPACITY {
            return Err("delayed notification registration capacity is exhausted".into());
        }
        let inner = Arc::new(DelayedNotifierInner {
            notifier,
            deadline: Mutex::new(None),
            timer: Arc::clone(self),
        });
        state.registrations.push(Arc::downgrade(&inner));
        Ok(DelayedNotifier(inner))
    }

    fn collect_due(state: &mut TimerState, now: Instant) -> (Vec<WorkerNotifier>, Option<Instant>) {
        let mut due = Vec::new();
        let mut next = None::<Instant>;
        state.registrations.retain(|entry| {
            let Some(inner) = entry.upgrade() else {
                return false;
            };
            let mut deadline = inner.deadline.lock().unwrap_or_else(|p| p.into_inner());
            if deadline.is_some_and(|at| at <= now) {
                deadline.take();
                due.push(inner.notifier.clone());
            } else if let Some(at) = *deadline {
                next = Some(next.map_or(at, |old| old.min(at)));
            }
            true
        });
        (due, next)
    }

    fn run(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            let (due, next) = Self::collect_due(&mut state, Instant::now());
            if !due.is_empty() {
                drop(state);
                for notifier in due {
                    notifier.notify();
                }
                state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            } else if let Some(at) = next {
                let (guard, _) = self
                    .changed
                    .wait_timeout(state, at.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|p| p.into_inner());
                state = guard;
            } else {
                state = self.changed.wait(state).unwrap_or_else(|p| p.into_inner());
            }
        }
    }
}

fn wake_timer() -> Result<Arc<WakeTimer>, String> {
    static TIMER: OnceLock<Mutex<Option<Arc<WakeTimer>>>> = OnceLock::new();
    let mut installed = TIMER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(timer) = &*installed {
        return Ok(Arc::clone(timer));
    }
    let timer = WakeTimer::new();
    let worker_timer = Arc::clone(&timer);
    thread::Builder::new()
        .name("golden-io-deadlines".into())
        .spawn(move || worker_timer.run())
        .map_err(|error| format!("failed to start delayed notification timer: {error}"))?;
    *installed = Some(Arc::clone(&timer));
    Ok(timer)
}

impl std::fmt::Debug for DelayedNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DelayedNotifier").field(&self.0.notifier).finish()
    }
}

#[cfg(test)]
mod tests;
