use super::super::frame::DmxFrame;
use golden_io::{RetainedPayload, WorkerNotifier};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, OnceLock,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum SendStatus {
    Pending,
    Sent,
    FailedUnconfirmed,
}
struct State {
    status: AtomicU8,
    failure: OnceLock<String>,
    wake: WorkerNotifier,
}
#[derive(Clone)]
pub(crate) struct SendReceipt(Arc<State>);
impl SendReceipt {
    pub(crate) fn status(&self) -> SendStatus {
        match self.0.status.load(Ordering::Acquire) {
            0 => SendStatus::Pending,
            1 => SendStatus::Sent,
            _ => SendStatus::FailedUnconfirmed,
        }
    }
    pub(crate) fn failure(&self) -> Option<&str> {
        self.0.failure.get().map(String::as_str)
    }
}
pub(super) struct Output {
    pub frame: RetainedPayload<DmxFrame>,
    state: Arc<State>,
}
impl Output {
    pub(super) fn new(frame: RetainedPayload<DmxFrame>, wake: WorkerNotifier) -> (Self, SendReceipt) {
        let state = Arc::new(State {
            status: AtomicU8::new(0),
            failure: OnceLock::new(),
            wake,
        });
        (
            Self {
                frame,
                state: state.clone(),
            },
            SendReceipt(state),
        )
    }
    pub(super) fn complete(self, result: Result<(), String>) {
        let status = match result {
            Ok(()) => SendStatus::Sent,
            Err(error) => {
                let _ = self.state.failure.set(super::diagnostic(error));
                SendStatus::FailedUnconfirmed
            }
        };
        self.state.status.store(status as u8, Ordering::Release);
        self.state.wake.notify();
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        if self
            .state
            .status
            .compare_exchange(
                0,
                SendStatus::FailedUnconfirmed as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.state.wake.notify();
        }
    }
}
