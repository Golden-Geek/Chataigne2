use std::sync::mpsc::{SyncSender, TrySendError};

use golden_protocol::ControlRequest;
use thiserror::Error;

/// Cloneable command handle backed by a channel. It contains no transport
/// state or mutex and can be passed to protocol adapters safely.
#[derive(Clone)]
pub struct EngineControlHandle {
    sender: SyncSender<ControlRequest>,
}

impl EngineControlHandle {
    pub const fn new(sender: SyncSender<ControlRequest>) -> Self {
        Self { sender }
    }

    pub fn try_send(&self, request: ControlRequest) -> Result<(), ControlHandleError> {
        self.sender.try_send(request).map_err(|error| match error {
            TrySendError::Full(_) => ControlHandleError::Backpressure,
            TrySendError::Disconnected(_) => ControlHandleError::Disconnected,
        })
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ControlHandleError {
    #[error("control command queue is full")]
    Backpressure,
    #[error("control plane is disconnected")]
    Disconnected,
}
