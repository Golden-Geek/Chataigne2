use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use golden_protocol::{ObservationMessage, PreviewChange, PreviewDelta, PreviewKey, ScopeId, ServerMessage};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub enum OutboundFrame {
    Message(ServerMessage),
    Binary(Vec<u8>),
}

#[derive(Default)]
pub struct TransportMetrics {
    preview_replacements: AtomicU64,
    preview_drops: AtomicU64,
    binary_replacements: AtomicU64,
    resync_markers: AtomicU64,
    reliable_backpressure: AtomicU64,
}

impl TransportMetrics {
    pub fn snapshot(&self) -> TransportMetricsSnapshot {
        TransportMetricsSnapshot {
            preview_replacements: self.preview_replacements.load(Ordering::Relaxed),
            preview_drops: self.preview_drops.load(Ordering::Relaxed),
            binary_replacements: self.binary_replacements.load(Ordering::Relaxed),
            resync_markers: self.resync_markers.load(Ordering::Relaxed),
            reliable_backpressure: self.reliable_backpressure.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransportMetricsSnapshot {
    pub preview_replacements: u64,
    pub preview_drops: u64,
    pub binary_replacements: u64,
    pub resync_markers: u64,
    pub reliable_backpressure: u64,
}

struct QueueState {
    reliable: VecDeque<ServerMessage>,
    previews: BTreeMap<PreviewKey, (u32, PreviewChange)>,
    resync: BTreeMap<ScopeId, u32>,
    binary: Option<Vec<u8>>,
}

pub struct ClientOutboundQueue {
    reliable_capacity: usize,
    preview_capacity: usize,
    state: Mutex<QueueState>,
    metrics: Arc<TransportMetrics>,
}

impl ClientOutboundQueue {
    pub fn new(
        reliable_capacity: usize,
        preview_capacity: usize,
        metrics: Arc<TransportMetrics>,
    ) -> Result<Self, QueueError> {
        if reliable_capacity == 0 || preview_capacity == 0 {
            return Err(QueueError::ZeroCapacity);
        }
        Ok(Self {
            reliable_capacity,
            preview_capacity,
            state: Mutex::new(QueueState {
                reliable: VecDeque::with_capacity(reliable_capacity),
                previews: BTreeMap::new(),
                resync: BTreeMap::new(),
                binary: None,
            }),
            metrics,
        })
    }

    pub fn enqueue_reliable(&self, message: ServerMessage) -> Result<(), QueueError> {
        let mut state = self.state.lock().expect("client queue lock poisoned");
        if state.reliable.len() == self.reliable_capacity {
            self.metrics.reliable_backpressure.fetch_add(1, Ordering::Relaxed);
            return Err(QueueError::ReliableBackpressure);
        }
        state.reliable.push_back(message);
        Ok(())
    }

    pub fn enqueue_preview(&self, delta: PreviewDelta) {
        let mut state = self.state.lock().expect("client queue lock poisoned");
        for change in delta.changes {
            if let Some(existing) = state.previews.get_mut(&change.key) {
                *existing = (delta.sequence, change);
                self.metrics.preview_replacements.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            if state.previews.len() == self.preview_capacity {
                let oldest = state
                    .previews
                    .iter()
                    .min_by_key(|(_, (sequence, _))| *sequence)
                    .map(|(key, _)| key.clone())
                    .expect("capacity is non-zero");
                if let Some((sequence, dropped)) = state.previews.remove(&oldest) {
                    if state.resync.len() < self.preview_capacity {
                        state
                            .resync
                            .entry(dropped.key.scope)
                            .and_modify(|current| *current = (*current).max(sequence))
                            .or_insert(sequence);
                    }
                    self.metrics.preview_drops.fetch_add(1, Ordering::Relaxed);
                }
            }
            state.previews.insert(change.key.clone(), (delta.sequence, change));
        }
    }

    pub fn enqueue_binary_latest(&self, frame: Vec<u8>) {
        let mut state = self.state.lock().expect("client queue lock poisoned");
        if state.binary.replace(frame).is_some() {
            self.metrics.binary_replacements.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn drain(&self, maximum_frames: usize) -> Vec<OutboundFrame> {
        let mut state = self.state.lock().expect("client queue lock poisoned");
        let mut frames = Vec::with_capacity(maximum_frames.min(self.queued_len_locked(&state)));
        while frames.len() < maximum_frames {
            if let Some(message) = state.reliable.pop_front() {
                frames.push(OutboundFrame::Message(message));
                continue;
            }
            if let Some((scope, sequence)) = state.resync.pop_first() {
                self.metrics.resync_markers.fetch_add(1, Ordering::Relaxed);
                frames.push(OutboundFrame::Message(ServerMessage::Observation(
                    ObservationMessage::ResyncRequired {
                        scope,
                        after_sequence: sequence,
                    },
                )));
                continue;
            }
            if !state.previews.is_empty() {
                let sequence = state
                    .previews
                    .values()
                    .map(|(sequence, _)| *sequence)
                    .max()
                    .unwrap_or(0);
                let changes = std::mem::take(&mut state.previews)
                    .into_values()
                    .map(|(_, change)| change)
                    .collect();
                frames.push(OutboundFrame::Message(ServerMessage::Observation(
                    ObservationMessage::Preview(PreviewDelta { sequence, changes }),
                )));
                continue;
            }
            if let Some(binary) = state.binary.take() {
                frames.push(OutboundFrame::Binary(binary));
                continue;
            }
            break;
        }
        frames
    }

    pub fn queued_len(&self) -> usize {
        let state = self.state.lock().expect("client queue lock poisoned");
        self.queued_len_locked(&state)
    }

    fn queued_len_locked(&self, state: &QueueState) -> usize {
        state.reliable.len()
            + state.resync.len()
            + usize::from(!state.previews.is_empty())
            + usize::from(state.binary.is_some())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum QueueError {
    #[error("queue capacities must be non-zero")]
    ZeroCapacity,
    #[error("reliable control queue is full; caller must apply backpressure")]
    ReliableBackpressure,
}
