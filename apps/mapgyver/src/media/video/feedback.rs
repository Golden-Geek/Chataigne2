//! Latest scalar status, delivered by an owned wake rather than an engine polling rule.
use golden_core::{engine::NodeWaker, process_ctx::ProcessCtx};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Status {
    pub playing: bool,
    pub length_ms: f64,
}

#[derive(Default)]
struct State {
    pending: Option<Status>,
    last: Option<Status>,
    closed: bool,
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
    pub fn publish(&self, value: Status) {
        let Some(inbox) = self.state.upgrade() else { return };
        let Ok(mut state) = inbox.lock() else { return };
        if state.closed || self.waker.is_closed() || state.last == Some(value) {
            return;
        }
        state.pending = Some(value);
        state.last = Some(value);
        drop(state);
        if self.waker.wake().is_err()
            && let Ok(mut state) = inbox.lock()
        {
            state.last = None;
        }
    }
}
