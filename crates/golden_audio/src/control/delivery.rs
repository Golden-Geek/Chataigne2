use crate::{AudioError, AudioEvent};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Shared {
    cancelled: Mutex<bool>,
    changed: Condvar,
    ordered: bool,
}
impl Shared {
    fn signal(&self) {
        let guard = self.cancelled.lock().unwrap_or_else(|p| p.into_inner());
        drop(guard);
        self.changed.notify_all();
    }
    fn cancel(&self) {
        *self.cancelled.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.changed.notify_all();
    }
}

#[derive(Clone, Debug)]
pub(super) struct EventCancellation(Arc<Shared>);
impl EventCancellation {
    /// Forced engine shutdown abandons publication, preserving its buffered prefix.
    pub fn cancel_ordered(&self) {
        if self.0.ordered {
            self.0.cancel();
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct EventSender {
    sender: Option<SyncSender<AudioEvent>>,
    shared: Arc<Shared>,
}
impl EventSender {
    /// Only the non-realtime control worker may wait for ordered event capacity.
    pub fn publish(&self, mut event: AudioEvent) -> bool {
        let mut cancelled = self.shared.cancelled.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if *cancelled {
                drop(cancelled);
                drop(event);
                return false;
            }
            match self.sender.as_ref().expect("live audio event sender").try_send(event) {
                Ok(()) => {
                    self.shared.changed.notify_all();
                    return true;
                }
                Err(TrySendError::Full(returned)) if self.shared.ordered => {
                    event = returned;
                    cancelled = self.shared.changed.wait(cancelled).unwrap_or_else(|p| p.into_inner());
                }
                Err(TrySendError::Full(event) | TrySendError::Disconnected(event)) => {
                    drop(cancelled);
                    drop(event);
                    return false;
                }
            }
        }
    }
}
impl Drop for EventSender {
    fn drop(&mut self) {
        drop(self.sender.take());
        self.shared.signal();
    }
}

/// Bounded audio notifications. Ordered consumers must keep draining to permit
/// control progress; dropping the receiver cancels blocked event publication.
#[derive(Debug)]
pub struct AudioEventReceiver {
    receiver: Receiver<AudioEvent>,
    shared: Arc<Shared>,
}
impl AudioEventReceiver {
    pub(super) fn cancellation(&self) -> EventCancellation {
        EventCancellation(self.shared.clone())
    }
    #[must_use]
    pub fn try_recv(&self) -> Option<AudioEvent> {
        let guard = self.shared.cancelled.lock().unwrap_or_else(|p| p.into_inner());
        let result = self.receiver.try_recv().ok();
        drop(guard);
        if result.is_some() {
            self.shared.changed.notify_all();
        }
        result
    }
    #[must_use]
    pub fn drain(&self) -> Vec<AudioEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.try_recv() {
            events.push(event);
        }
        events
    }
    pub fn recv(&self) -> Result<AudioEvent, AudioError> {
        self.receive(None)
            .map(|event| event.expect("unbounded receive does not time out"))
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<AudioEvent>, AudioError> {
        self.receive(Some(timeout))
    }
    fn receive(&self, timeout: Option<Duration>) -> Result<Option<AudioEvent>, AudioError> {
        let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
        let mut cancelled = self.shared.cancelled.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    self.shared.changed.notify_all();
                    return Ok(Some(event));
                }
                Err(TryRecvError::Disconnected) => return Err(AudioError::shutting_down()),
                Err(TryRecvError::Empty) if *cancelled => return Err(AudioError::shutting_down()),
                Err(TryRecvError::Empty) => {}
            }
            if let Some(deadline) = deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                cancelled = self
                    .shared
                    .changed
                    .wait_timeout(cancelled, remaining)
                    .unwrap_or_else(|p| p.into_inner())
                    .0;
            } else {
                cancelled = self.shared.changed.wait(cancelled).unwrap_or_else(|p| p.into_inner());
            }
        }
    }
}
impl Drop for AudioEventReceiver {
    fn drop(&mut self) {
        self.shared.cancel();
    }
}

pub(super) fn event_channel(capacity: usize, ordered: bool) -> (EventSender, AudioEventReceiver) {
    assert!(capacity > 0);
    let (sender, receiver) = mpsc::sync_channel(capacity);
    let shared = Arc::new(Shared {
        cancelled: Mutex::new(false),
        changed: Condvar::new(),
        ordered,
    });
    (
        EventSender {
            sender: Some(sender),
            shared: shared.clone(),
        },
        AudioEventReceiver { receiver, shared },
    )
}

#[cfg(test)]
mod tests;
