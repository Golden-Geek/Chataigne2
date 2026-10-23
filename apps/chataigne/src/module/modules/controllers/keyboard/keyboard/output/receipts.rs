use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Completed(Result<String, String>),
    NotSent,
    Unconfirmed,
}

struct State {
    result: Mutex<Option<Outcome>>,
    complete: AtomicBool,
    started: AtomicBool,
    notifier: golden_io::WorkerNotifier,
    source: RetainedPayload<Source>,
}

pub(super) struct Receipt(Arc<State>);
impl Receipt {
    pub(super) fn attempted(&self) -> bool {
        self.0.started.load(Ordering::Acquire)
    }
    pub(super) fn take(&self) -> Option<Outcome> {
        self.0.result.lock().unwrap_or_else(|error| error.into_inner()).take()
    }
    pub(super) fn complete(&self) -> bool {
        self.0.complete.load(Ordering::Acquire)
    }
    pub(super) fn source(&self) -> RetainedPayload<Source> {
        self.0.source.clone()
    }
}

pub(super) struct Command {
    state: Arc<State>,
    pub(super) armed: bool,
}
impl Command {
    pub(super) fn new(source: RetainedPayload<Source>, notifier: golden_io::WorkerNotifier) -> (Self, Receipt) {
        let state = Arc::new(State {
            result: Mutex::new(None),
            complete: AtomicBool::new(false),
            started: AtomicBool::new(false),
            notifier,
            source,
        });
        (
            Self {
                state: state.clone(),
                armed: true,
            },
            Receipt(state),
        )
    }
    pub(super) fn current(&self) -> bool {
        self.state.source.live()
    }
    pub(super) fn prepare(&self) -> Result<Request, String> {
        self.state.source.prepare()
    }
    pub(super) fn started(&self) {
        self.state.started.store(true, Ordering::Release);
    }
    pub(super) fn finish(&self, result: Outcome) {
        let mut slot = self.state.result.lock().unwrap_or_else(|error| error.into_inner());
        if self.state.complete.load(Ordering::Acquire) {
            return;
        }
        *slot = Some(result);
        self.state.complete.store(true, Ordering::Release);
        drop(slot);
        self.state.notifier.notify();
    }
}
impl Drop for Command {
    fn drop(&mut self) {
        if self.armed && !self.state.complete.load(Ordering::Acquire) {
            self.finish(if self.state.started.load(Ordering::Acquire) {
                Outcome::Unconfirmed
            } else {
                Outcome::NotSent
            });
        }
    }
}
