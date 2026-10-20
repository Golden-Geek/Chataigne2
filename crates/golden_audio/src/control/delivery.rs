use crate::{AudioError, AudioEvent};
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

pub(crate) const EVENT_QUEUE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const EVENT_PAYLOAD_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
struct Entry {
    event: AudioEvent,
    weight: usize,
}
#[derive(Debug)]
struct State {
    queue: VecDeque<Entry>,
    retained_bytes: usize,
    senders: usize,
    cancelled: bool,
}
#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    ordered: bool,
    item_capacity: usize,
    byte_capacity: usize,
}
impl Shared {
    fn cancel(&self) {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).cancelled = true;
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

#[derive(Debug)]
pub(super) struct EventSender {
    shared: Arc<Shared>,
}
impl EventSender {
    /// Preparation and capacity waits belong to the non-realtime control worker.
    pub fn publish(&self, mut event: AudioEvent) -> bool {
        event.prepare_delivery();
        let weight = event.retained_bytes();
        if weight > EVENT_PAYLOAD_BYTES.min(self.shared.byte_capacity) {
            return false;
        }
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if state.cancelled {
                drop(state);
                return false;
            }
            if state.queue.len() < self.shared.item_capacity
                && weight <= self.shared.byte_capacity - state.retained_bytes
            {
                state.queue.push_back(Entry { event, weight });
                state.retained_bytes += weight;
                self.shared.changed.notify_all();
                return true;
            }
            if !self.shared.ordered {
                drop(state);
                return false;
            }
            state = self.shared.changed.wait(state).unwrap_or_else(|p| p.into_inner());
        }
    }
}
impl Clone for EventSender {
    fn clone(&self) -> Self {
        self.shared.state.lock().unwrap_or_else(|p| p.into_inner()).senders += 1;
        Self {
            shared: self.shared.clone(),
        }
    }
}
impl Drop for EventSender {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap_or_else(|p| p.into_inner()).senders -= 1;
        self.shared.changed.notify_all();
    }
}

/// Bounded audio notifications. Ordered consumers must keep draining to permit
/// control progress; dropping the receiver cancels blocked event publication.
#[derive(Debug)]
pub struct AudioEventReceiver {
    shared: Arc<Shared>,
}
impl AudioEventReceiver {
    pub(super) fn cancellation(&self) -> EventCancellation {
        EventCancellation(self.shared.clone())
    }
    #[must_use]
    pub fn try_recv(&self) -> Option<AudioEvent> {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        let entry = state.queue.pop_front()?;
        state.retained_bytes -= entry.weight;
        self.shared.changed.notify_all();
        Some(entry.event)
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
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(entry) = state.queue.pop_front() {
                state.retained_bytes -= entry.weight;
                self.shared.changed.notify_all();
                return Ok(Some(entry.event));
            }
            if state.senders == 0 || state.cancelled {
                return Err(AudioError::shutting_down());
            }
            if let Some(deadline) = deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                state = self
                    .shared
                    .changed
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|p| p.into_inner())
                    .0;
            } else {
                state = self.shared.changed.wait(state).unwrap_or_else(|p| p.into_inner());
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
    event_channel_with_bytes(capacity, ordered, EVENT_QUEUE_BYTES)
}
fn event_channel_with_bytes(capacity: usize, ordered: bool, bytes: usize) -> (EventSender, AudioEventReceiver) {
    assert!(capacity > 0 && bytes > 0);
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            queue: VecDeque::new(),
            retained_bytes: 0,
            senders: 1,
            cancelled: false,
        }),
        changed: Condvar::new(),
        ordered,
        item_capacity: capacity,
        byte_capacity: bytes,
    });
    (EventSender { shared: shared.clone() }, AudioEventReceiver { shared })
}

#[cfg(test)]
mod tests;
