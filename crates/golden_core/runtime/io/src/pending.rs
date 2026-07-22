use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SendError, TryRecvError},
};

/// Sender half of an event channel that exposes pending work without polling.
pub struct PendingSender<T> {
    sender: mpsc::Sender<T>,
    pending: Arc<AtomicBool>,
}

/// Receiver half of an event channel that exposes pending work without polling.
pub struct PendingReceiver<T> {
    receiver: mpsc::Receiver<T>,
    pending: Arc<AtomicBool>,
}

/// Creates an unbounded worker channel with a cheap event-ready signal.
///
/// Consumers clear the signal before draining. A concurrent producer then restores the signal,
/// so work arriving during a drain remains observable by the next scheduler pass.
pub fn pending_channel<T>() -> (PendingSender<T>, PendingReceiver<T>) {
    let pending = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    (
        PendingSender {
            sender,
            pending: Arc::clone(&pending),
        },
        PendingReceiver { receiver, pending },
    )
}

impl<T> PendingSender<T> {
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.pending.store(true, Ordering::Release);
        self.sender.send(value)
    }
}

impl<T> PendingReceiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    /// Clears the ready signal before the caller drains queued events.
    pub fn clear_pending(&self) {
        self.pending.store(false, Ordering::Release);
    }
}
