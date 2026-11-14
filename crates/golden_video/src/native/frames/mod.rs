//! Bounded GPU image publication. Only NT handles/immutable leases cross workers.
use super::{AdapterId, inputs::surface::Surface};
use crate::TextureDescriptor;
use crate::scheduling::{FrameBuffer, FramePosition, FrameWindow};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};
use std::time::Instant;
pub(super) mod budget;
mod reader;
pub use budget::{
    GpuFrameAllocation, GpuFrameBudget, GpuFrameBudgetSnapshot, SourceUploadSnapshot, SourceUploadTotals,
};
pub(super) use reader::{Imports, ReadLease};
#[cfg(feature = "ffmpeg")]
mod hap;
#[cfg(feature = "ffmpeg")]
mod producer;
#[cfg(feature = "ffmpeg")]
pub use producer::{GpuFrameError, GpuFramePublisher};

struct Fault {
    code: AtomicI32,
    wake: Option<golden_io::WorkerNotifier>,
}
impl Fault {
    fn wake(&self) {
        if let Some(wake) = &self.wake {
            wake.notify();
        }
    }
    fn record(&self, code: i32) {
        self.code.store(code, Ordering::Release);
        self.wake();
    }
}

/// Completed immutable RGBA publication. It retains the source slot and its storage budget.
/// It contains no COM interface and cannot be constructed from an uncompleted GPU write.
#[derive(Clone)]
pub struct GpuImage {
    pub(super) software_upload: Option<super::SoftwareUploadTrace>,
    // Share opt-in diagnostic identity across immutable image clones.
    pub(super) submission: Option<Arc<crate::runtime::CompletionToken>>,
    surface: Arc<Surface>,
    adapter: AdapterId,
    fault: Arc<Fault>,
}
impl GpuImage {
    /// Declared encoded RGB/alpha contract of this completed image.
    pub fn descriptor(&self) -> TextureDescriptor {
        self.surface.descriptor
    }
    /// Exact producer adapter. Cross-adapter imports are rejected.
    pub fn adapter(&self) -> AdapterId {
        self.adapter
    }
    pub(super) fn fail(&self, code: i32) {
        self.fault.record(code);
    }
}

#[derive(Default)]
struct State {
    sequence: u64,
    image: Option<GpuImage>,
    scheduled: FrameBuffer<(u64, GpuImage, Option<u64>)>,
    epoch: Option<golden_time::TimeEpoch>,
    withheld: bool,
}
/// One coherent nonblocking renderer observation. None image explicitly withdraws pixels.
pub(super) struct Sample {
    pub sequence: u64,
    pub image: Option<GpuImage>,
    pub frame: Option<super::TimedFrameTrace>,
    pub position: Option<FramePosition>,
}

/// Nonblocking source clock projection at a renderer's common input-sampling instant.
pub trait GpuFrameClock: Send + Sync {
    /// None retains the renderer's prior pixels rather than waiting on another owner.
    fn position_at(&self, instant: Instant) -> Option<FramePosition>;
}

/// Result of bounded scheduled publication. Busy returns the completed image for retry.
pub enum GpuFramePublication {
    /// Displaced storage must be destroyed after releasing the outer owner lock.
    Published([Option<GpuImage>; 3]),
    /// Two unexpired windows are already retained; no publication occurred.
    Busy(GpuImage),
}

/// Fixed-descriptor GPU source with immediate or bounded timestamp-based publication.
#[derive(Clone)]
pub struct GpuFrameStream {
    id: uuid::Uuid,
    descriptor: TextureDescriptor,
    adapter: AdapterId,
    state: Arc<Mutex<State>>,
    clock: Option<Arc<dyn GpuFrameClock>>,
    timing: Option<super::timing::Clock>,
}
impl std::fmt::Debug for GpuFrameStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuFrameStream")
            .field("descriptor", &self.descriptor)
            .field("adapter", &self.adapter)
            .finish_non_exhaustive()
    }
}
impl GpuFrameStream {
    /// Create an empty source; the producer must admit storage before publishing images.
    pub fn new(descriptor: TextureDescriptor, adapter: AdapterId) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            descriptor,
            adapter,
            state: Default::default(),
            clock: None,
            timing: None,
        }
    }
    /// Opaque publication lifetime, shared by clones and independent of renderer generations.
    pub fn id(&self) -> uuid::Uuid {
        self.id
    }

    /// Immutable sampled RGB contract installed with a plan.
    pub fn descriptor(&self) -> TextureDescriptor {
        self.descriptor
    }
    /// Adapter required by every source image and renderer import.
    pub fn adapter(&self) -> AdapterId {
        self.adapter
    }
    /// Release obsolete committed slots before allocating an image for a new clock epoch.
    /// Existing renderers retain their copied pixels; this is not an explicit withdrawal.
    /// Dispose the returned immutable images after releasing any outer owner lock.
    pub fn begin_epoch(&self, epoch: golden_time::TimeEpoch) -> Result<[Option<GpuImage>; 3], &'static str> {
        let mut state = self.state.lock().map_err(|_| "GPU image stream poisoned")?;
        if state.epoch == Some(epoch) {
            return Ok([None, None, None]);
        }
        state.epoch = Some(epoch);
        state.withheld = true;
        let old = state.image.take();
        let [a, b] = state.scheduled.clear();
        drop(state);
        Ok([old, a.map(|(_, image, _)| image), b.map(|(_, image, _)| image)])
    }
    /// Publish only a completed image, or withdraw. Dispose displaced storage outside locks.
    pub fn publish(&self, image: Option<GpuImage>) -> Result<(), &'static str> {
        self.replace(image).map(drop)
    }
    /// Replace under an owner's epoch fence, returning old handle/storage ownership so it
    /// can be disposed after that outer control lock is released.
    pub fn replace(&self, image: Option<GpuImage>) -> Result<[Option<GpuImage>; 3], &'static str> {
        if image
            .as_ref()
            .is_some_and(|image| image.descriptor() != self.descriptor || image.adapter != self.adapter)
        {
            return Err("GPU stream image contract changed");
        }
        let mut state = self.state.lock().map_err(|_| "GPU image stream poisoned")?;
        let sequence = state.sequence.checked_add(1).ok_or("GPU stream sequence exhausted")?;
        let old = std::mem::replace(&mut state.image, image);
        let scheduled = state.scheduled.clear();
        state.sequence = sequence;
        state.withheld = false;
        drop(state);
        let [a, b] = scheduled;
        Ok([old, a.map(|(_, image, _)| image), b.map(|(_, image, _)| image)])
    }
    /// Queue a completed frame without making it visible before its presentation window.
    pub fn replace_scheduled(
        &self,
        image: GpuImage,
        window: FrameWindow,
        now: FramePosition,
    ) -> Result<GpuFramePublication, &'static str> {
        if self.clock.is_none() || image.descriptor() != self.descriptor || image.adapter != self.adapter {
            return Err("scheduled GPU stream contract changed");
        }
        let mut state = self.state.lock().map_err(|_| "GPU image stream poisoned")?;
        let sequence = state.sequence.checked_add(1).ok_or("GPU stream sequence exhausted")?;
        let published = self.timing.and_then(|clock| clock.stamp());
        let result = match state.scheduled.publish(window, (sequence, image, published), now) {
            Ok(retired) => {
                state.sequence = sequence;
                state.withheld = false;
                let [a, b] = retired;
                GpuFramePublication::Published([
                    state.image.take(),
                    a.map(|(_, image, _)| image),
                    b.map(|(_, image, _)| image),
                ])
            }
            Err((_, image, _)) => GpuFramePublication::Busy(image),
        };
        drop(state);
        Ok(result)
    }
    #[cfg(test)]
    pub(super) fn sample(&self) -> Option<Sample> {
        self.sample_at(Instant::now())
    }
    pub(super) fn sample_at(&self, instant: Instant) -> Option<Sample> {
        // A stationary/immediate image has no presentation deadline or clock dependency.
        {
            let state = self.state.try_lock().ok()?;
            if state.withheld {
                return None;
            }
            if state.scheduled.is_empty() {
                return Some(state.immediate());
            }
        }
        // Query the app-neutral clock before the stream lock: publishers fence in the opposite order.
        let position = match &self.clock {
            Some(clock) => Some(clock.position_at(instant)?),
            None => None,
        };
        let state = self.state.try_lock().ok()?;
        if state.withheld {
            return None;
        }
        if !state.scheduled.is_empty() {
            let (window, (sequence, image, published)) = state.scheduled.sample(position?)?;
            return Some(Sample {
                sequence: *sequence,
                image: Some(image.clone()),
                position,
                frame: Some(super::TimedFrameTrace {
                    pts_nanos: window.pts().as_nanos(),
                    start_nanos: window.start().as_nanos(),
                    end_nanos: window.end().map(|end| end.as_nanos()),
                    publication_qpc: *published,
                }),
            });
        }
        Some(state.immediate())
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        size_of::<State>() + 2 * size_of::<usize>()
    }
}

impl State {
    fn immediate(&self) -> Sample {
        Sample {
            sequence: self.sequence,
            image: self.image.clone(),
            frame: None,
            position: None,
        }
    }
}

#[cfg(test)]
pub(in crate::native) mod tests;
