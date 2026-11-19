//! Bounded non-realtime PCM handoff into one SDK publication worker.
use golden_io::WorkerNotifier;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
    time::Instant,
};

const BLOCKS: usize = 12;
const MAX_SAMPLES: usize = 256 * 1024;

/// An app-fed PCM input for one NDI output. A replacement publisher gets a fresh queue.
#[derive(Clone, Default)]
pub struct NdiOutputAudio(Arc<Mutex<Weak<Inbox>>>);

impl PartialEq for NdiOutputAudio {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for NdiOutputAudio {}

impl std::fmt::Debug for NdiOutputAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NdiOutputAudio").finish_non_exhaustive()
    }
}

pub(crate) struct AudioBlock {
    pub sample_rate: u32,
    pub channels: usize,
    pub starts_at: Instant,
    pub samples: Vec<f32>,
}

pub(crate) struct Inbox {
    pending: Mutex<VecDeque<AudioBlock>>,
    wake: WorkerNotifier,
}

impl Inbox {
    pub(crate) fn take(&self) -> Option<AudioBlock> {
        self.pending.lock().unwrap().pop_front()
    }
}

impl NdiOutputAudio {
    /// Does an output worker currently have a live receiver?
    pub fn is_connected(&self) -> bool {
        self.0.lock().unwrap().strong_count() != 0
    }

    /// Queue owned, tightly packed planar samples in NDI reference levels. Older pending
    /// blocks are discarded at capacity. Call only from a non-realtime producer.
    pub fn publish(&self, sample_rate: u32, channels: usize, starts_at: Instant, samples: Vec<f32>) {
        if !(1..=super::audio::MAX_AUDIO_CHANNELS).contains(&channels)
            || sample_rate == 0
            || samples.is_empty()
            || samples.len() > MAX_SAMPLES
            || !samples.len().is_multiple_of(channels)
        {
            return;
        }
        let Some(inbox) = self.0.lock().unwrap().upgrade() else {
            return;
        };
        let mut pending = inbox.pending.lock().unwrap();
        if pending.len() == BLOCKS {
            pending.pop_front();
        }
        pending.push_back(AudioBlock {
            sample_rate,
            channels,
            starts_at,
            samples,
        });
        drop(pending);
        inbox.wake.notify();
    }

    pub(crate) fn attach(&self, wake: WorkerNotifier) -> Arc<Inbox> {
        let inbox = Arc::new(Inbox {
            pending: Mutex::new(VecDeque::with_capacity(BLOCKS)),
            wake,
        });
        *self.0.lock().unwrap() = Arc::downgrade(&inbox);
        inbox
    }
}
