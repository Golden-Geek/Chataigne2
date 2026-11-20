//! Bounded, authenticated preview pulls. A host worker owns rendering and JPEG encoding.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_SOURCES: usize = 8;
const MAX_SOURCE_BYTES: usize = 512;
const MAX_JPEG_BYTES: usize = 2 * 1024 * 1024;
const DEMAND_LEASE: Duration = Duration::from_millis(1200);

/// Latest-frame mailbox for browser previews, partitioned by project and opaque source key.
#[derive(Clone, Default)]
pub struct BrowserPreviews(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    generation: u64,
    next_id: u64,
    streams: HashMap<String, Lease>,
    notifier: Option<golden_io::WorkerNotifier>,
}

struct Lease {
    stream: BrowserPreviewStream,
    until: Instant,
}

/// One active source demand. An expired handle cannot publish into a replacement stream.
#[derive(Clone)]
pub struct BrowserPreviewStream {
    id: u64,
    source: Arc<str>,
    latest: Arc<Mutex<Option<Frame>>>,
}

#[derive(Clone)]
pub(super) struct Frame {
    sequence: u64,
    pub tag: String,
    pub jpeg: Arc<[u8]>,
}

impl BrowserPreviewStream {
    /// Stream incarnation within this server process.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// App-defined source key from the browser preview component.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Replaces the previous encoded frame. A slow browser never accumulates a frame queue.
    pub fn publish_jpeg(&self, jpeg: Vec<u8>) {
        if jpeg.is_empty() || jpeg.len() > MAX_JPEG_BYTES {
            return;
        }
        let mut latest = self.latest.lock().expect("browser preview frame lock");
        let sequence = latest.as_ref().map_or(1, |frame| frame.sequence.wrapping_add(1));
        *latest = Some(Frame {
            sequence,
            tag: format!("\"{}-{sequence}\"", self.id),
            jpeg: jpeg.into(),
        });
    }
}

impl State {
    fn expire(&mut self, generation: u64, now: Instant) {
        if generation > self.generation {
            self.generation = generation;
            self.streams.clear();
        }
        self.streams.retain(|_, lease| lease.until > now);
    }
}

impl BrowserPreviews {
    /// Wakes the owning worker when a source first becomes visible.
    pub fn set_notifier(&self, notifier: golden_io::WorkerNotifier) {
        self.0.lock().expect("browser preview demand lock").notifier = Some(notifier);
    }

    /// Snapshot active sources in the current project. The caller should retire output work
    /// after the last lease expires, including clients that closed without a final request.
    pub fn active(&self, generation: u64) -> Vec<BrowserPreviewStream> {
        let mut state = self.0.lock().expect("browser preview demand lock");
        state.expire(generation, Instant::now());
        if generation != state.generation {
            return Vec::new();
        }
        let mut active: Vec<_> = state.streams.values().map(|lease| lease.stream.clone()).collect();
        active.sort_unstable_by_key(BrowserPreviewStream::id);
        active
    }

    pub(super) fn request(&self, generation: u64, source: &[u8]) -> Result<Option<Frame>, &'static str> {
        let source = std::str::from_utf8(source).map_err(|_| "preview source must be UTF-8")?;
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err("invalid browser preview source length");
        }
        let now = Instant::now();
        let (stream, notifier) = {
            let mut state = self.0.lock().expect("browser preview demand lock");
            state.expire(generation, now);
            if generation != state.generation {
                return Ok(None);
            }
            if let Some(lease) = state.streams.get_mut(source) {
                lease.until = now + DEMAND_LEASE;
                (lease.stream.clone(), None)
            } else {
                if state.streams.len() >= MAX_SOURCES {
                    return Ok(None);
                }
                state.next_id = state.next_id.wrapping_add(1);
                let stream = BrowserPreviewStream {
                    id: state.next_id,
                    source: source.into(),
                    latest: Default::default(),
                };
                state.streams.insert(
                    source.to_owned(),
                    Lease {
                        stream: stream.clone(),
                        until: now + DEMAND_LEASE,
                    },
                );
                (stream, state.notifier.clone())
            }
        };
        if let Some(notifier) = notifier {
            notifier.notify();
        }
        Ok(stream.latest.lock().expect("browser preview frame lock").clone())
    }
}

#[cfg(test)]
mod tests;
