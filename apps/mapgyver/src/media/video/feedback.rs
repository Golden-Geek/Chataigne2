//! Latest scalar status, delivered by an owned wake rather than an engine polling rule.
use golden_core::{engine::NodeWaker, process_ctx::ProcessCtx};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Debug)]
pub(crate) struct Status {
    pub playing: bool,
    pub length_ms: f64,
    pub position_ms: f64,
    pub input_time: Option<golden_core::engine::EngineTime>,
    pub guard: golden_io::RevisionGuard,
    pub range_guard: golden_io::RevisionGuard,
}
impl Status {
    fn same_values(&self, other: &Self) -> bool {
        self.playing == other.playing
            && self.length_ms == other.length_ms
            && self.position_ms == other.position_ms
            && self.input_time == other.input_time
    }
}

#[derive(Default)]
struct State {
    pending: Option<Status>,
    last: Option<Status>,
    closed: bool,
    sequence: u64,
}

#[derive(Default)]
pub(crate) struct Inbox(Option<Arc<Mutex<State>>>);
impl Inbox {
    pub fn register(&mut self, ctx: &ProcessCtx) -> Result<Publisher, String> {
        let waker = ctx.node_waker().map_err(|error| error.to_string())?;
        // A restored node receives a new inbox; its retired producer cannot reach this lifetime.
        self.close();
        let state = Arc::new(Mutex::new(State::default()));
        let publisher = Publisher {
            state: Arc::downgrade(&state),
            waker,
        };
        self.0 = Some(state);
        Ok(publisher)
    }
    pub fn take(&self) -> Option<Status> {
        self.0.as_ref()?.lock().ok()?.pending.take()
    }
    pub fn close(&self) {
        let Some(state) = &self.0 else { return };
        if let Ok(mut state) = state.lock() {
            state.closed = true;
            state.pending = None;
        }
    }
}
impl Drop for Inbox {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Clone)]
pub(crate) struct Publisher {
    state: Weak<Mutex<State>>,
    waker: NodeWaker,
}
impl Publisher {
    pub fn publish(&self, sequence: u64, value: Status) {
        let Some(inbox) = self.state.upgrade() else { return };
        let Ok(mut state) = inbox.lock() else { return };
        if state.closed || self.waker.is_closed() || sequence <= state.sequence {
            return;
        }
        state.sequence = sequence;
        if state
            .last
            .as_ref()
            .is_some_and(|last| last.same_values(&value) && last.guard.is_current() && last.range_guard.is_current())
        {
            // A wake coalesced with the currently running callback may leave a pending sample.
            let retry = state.pending.is_some();
            drop(state);
            if retry {
                let _ = self.waker.wake();
            }
            return;
        }
        state.pending = Some(value.clone());
        state.last = Some(value);
        drop(state);
        if self.waker.wake().is_err()
            && let Ok(mut state) = inbox.lock()
            && state.sequence == sequence
        {
            state.last = None;
        }
    }
}
