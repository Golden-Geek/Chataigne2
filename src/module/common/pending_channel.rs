use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SendError, TryRecvError},
    Arc,
};

/// Sender half of a pending-aware channel.  Calling `send` sets the flag
/// before forwarding to the underlying `mpsc::Sender`, allowing the receiver
/// side to cheaply check whether any event is waiting without draining.
pub(crate) struct PendingSender<T> {
    tx: mpsc::Sender<T>,
    flag: Arc<AtomicBool>,
}

/// Receiver half of a pending-aware channel.  Exposes `has_pending` / `clear_pending`
/// so a node's `needs_update()` can gate the update without polling the channel.
pub(crate) struct PendingReceiver<T> {
    rx: mpsc::Receiver<T>,
    flag: Arc<AtomicBool>,
}

pub(crate) fn pending_channel<T>() -> (PendingSender<T>, PendingReceiver<T>) {
    let flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    (
        PendingSender { tx, flag: Arc::clone(&flag) },
        PendingReceiver { rx, flag },
    )
}

impl<T> PendingSender<T> {
    pub(crate) fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.flag.store(true, Ordering::Relaxed);
        self.tx.send(value)
    }
}

impl<T> PendingReceiver<T> {
    pub(crate) fn try_recv(&self) -> Result<T, TryRecvError> {
        self.rx.try_recv()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Reset the pending flag.  Call this **before** draining the channel so
    /// any events the worker sends during the drain are not lost.
    pub(crate) fn clear_pending(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }
}

// Safety: PendingSender / PendingReceiver mirror the Send/!Send characteristics
// of the underlying mpsc halves.
unsafe impl<T: Send> Send for PendingSender<T> {}
unsafe impl<T: Send> Send for PendingReceiver<T> {}
