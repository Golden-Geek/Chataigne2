//! Bounded, authenticated preview pulls. A host worker owns rendering and JPEG encoding.
use golden_protocol::UiBrowserPreviewQuality::{self, High, Low};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_SOURCE_BYTES: usize = 512;
const MAX_JPEG_BYTES: usize = 2 * 1024 * 1024;
const DEMAND_LEASE: Duration = Duration::from_millis(2400);

/// Encoder settings for one representation of a browser preview.
pub struct BrowserPreviewEncoding {
    /// Maximum encoded width, preserving content aspect.
    pub width: u32,
    /// Maximum encoded height, preserving content aspect.
    pub height: u32,
    /// JPEG quality on a scale from 1 to 100.
    pub jpeg_quality: u8,
}

const PROFILES: [(BrowserPreviewEncoding, Duration); 2] = [
    (
        BrowserPreviewEncoding {
            width: 320,
            height: 180,
            jpeg_quality: 50,
        },
        Duration::from_millis(120),
    ),
    (
        BrowserPreviewEncoding {
            width: 640,
            height: 360,
            jpeg_quality: 75,
        },
        Duration::from_millis(60),
    ),
];

fn profile_index(quality: UiBrowserPreviewQuality) -> usize {
    match quality {
        Low => 0,
        High => 1,
    }
}

/// Latest-frame mailbox for browser previews, partitioned by project and opaque source key.
#[derive(Clone, Default)]
pub struct BrowserPreviews(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    generation: u64,
    next_id: u64,
    streams: HashMap<String, BrowserPreviewStream>,
    notifier: Option<golden_io::WorkerNotifier>,
}

#[derive(Default)]
struct Variant {
    until: Option<Instant>,
    encoded_at: Option<Instant>,
    sequence: u64,
    latest: Option<Frame>,
}

impl Variant {
    fn active(&self, now: Instant) -> bool {
        self.until.is_some_and(|until| until > now)
    }
}

/// One active source demand. An expired handle cannot publish into a replacement stream.
#[derive(Clone)]
pub struct BrowserPreviewStream {
    id: u64,
    source: Arc<str>,
    variants: Arc<Mutex<[Variant; PROFILES.len()]>>,
}

#[derive(Clone)]
pub(super) struct Frame {
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

    /// Encodes the current image for each due representation with a live lease. The caller
    /// owns one publishing worker per stream. Encoding happens outside the request lock.
    pub fn publish_frame<E>(&self, encode: impl FnMut(&BrowserPreviewEncoding) -> Result<Vec<u8>, E>) -> Result<(), E> {
        self.publish_frame_at(Instant::now(), encode)
    }

    fn publish_frame_at<E>(
        &self,
        now: Instant,
        mut encode: impl FnMut(&BrowserPreviewEncoding) -> Result<Vec<u8>, E>,
    ) -> Result<(), E> {
        for (index, (encoding, interval)) in PROFILES.iter().enumerate() {
            {
                let mut variants = self.variants.lock().expect("browser preview frame lock");
                let variant = &mut variants[index];
                if !variant.active(now) || variant.encoded_at.is_some_and(|at| now - at < *interval) {
                    continue;
                }
                variant.encoded_at = Some(now);
            }
            let jpeg = encode(encoding)?;
            if jpeg.is_empty() || jpeg.len() > MAX_JPEG_BYTES {
                continue;
            }
            let mut variants = self.variants.lock().expect("browser preview frame lock");
            let variant = &mut variants[index];
            if !variant.active(Instant::now())
                || variant.encoded_at != Some(now)
                || variant
                    .latest
                    .as_ref()
                    .is_some_and(|frame| frame.jpeg.as_ref() == jpeg.as_slice())
            {
                continue;
            }
            variant.sequence = variant.sequence.wrapping_add(1);
            variant.latest = Some(Frame {
                tag: format!("\"{}-{index}-{}\"", self.id, variant.sequence),
                jpeg: jpeg.into(),
            });
        }
        Ok(())
    }
}

impl State {
    fn expire(&mut self, generation: u64, now: Instant) {
        if generation > self.generation {
            self.generation = generation;
            self.streams.clear();
        }
        self.streams.retain(|_, stream| {
            let mut variants = stream.variants.lock().expect("browser preview frame lock");
            for variant in variants.iter_mut().filter(|variant| !variant.active(now)) {
                variant.latest = None;
                variant.encoded_at = None;
            }
            variants.iter().any(|variant| variant.active(now))
        });
    }
}

impl BrowserPreviews {
    /// Maximum number of distinct sources with live browser preview leases.
    pub const SOURCE_LIMIT: usize = 8;

    /// Maximum browser preview backing size. Smaller variants share this rendered image.
    pub const CAPTURE_SIZE: (u32, u32) = (PROFILES[1].0.width, PROFILES[1].0.height);

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
        let mut active: Vec<_> = state.streams.values().cloned().collect();
        active.sort_unstable_by_key(BrowserPreviewStream::id);
        active
    }

    pub(super) fn request(
        &self,
        generation: u64,
        source: &str,
        quality: UiBrowserPreviewQuality,
    ) -> Result<Option<Frame>, &'static str> {
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err("invalid browser preview source length");
        }
        let now = Instant::now();
        let (frame, notifier) = {
            let mut state = self.0.lock().expect("browser preview demand lock");
            state.expire(generation, now);
            if generation != state.generation {
                return Ok(None);
            }
            let is_new = !state.streams.contains_key(source);
            if is_new {
                if state.streams.len() >= Self::SOURCE_LIMIT {
                    return Ok(None);
                }
                state.next_id = state.next_id.wrapping_add(1);
                let stream = BrowserPreviewStream {
                    id: state.next_id,
                    source: source.into(),
                    variants: Default::default(),
                };
                state.streams.insert(source.to_owned(), stream);
            }
            let mut variants = state.streams[source]
                .variants
                .lock()
                .expect("browser preview frame lock");
            let variant = &mut variants[profile_index(quality)];
            variant.until = Some(now + DEMAND_LEASE);
            (
                variant.latest.clone(),
                if is_new { state.notifier.clone() } else { None },
            )
        };
        if let Some(notifier) = notifier {
            notifier.notify();
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests;
