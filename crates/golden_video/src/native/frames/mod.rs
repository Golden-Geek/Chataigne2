//! Bounded GPU image publication. Only NT handles/immutable leases cross workers.
use super::{
    AdapterId,
    inputs::{budget, surface::Surface},
};
use crate::TextureDescriptor;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};
mod reader;
pub(super) use reader::{Imports, ReadLease};
#[cfg(feature = "ffmpeg")]
mod producer;
#[cfg(feature = "ffmpeg")]
pub use producer::{GpuFrameError, GpuFramePublisher};

/// Shared admission for decoder pools, conversion images and retained bridge slots.
/// Renderer-owned targets require separate admission from the same adapter allowance.
#[derive(Clone)]
pub struct GpuFrameBudget(budget::Budget);
impl GpuFrameBudget {
    #[cfg(feature = "ffmpeg")]
    pub(crate) fn reserve(&self, bytes: u64) -> Option<Arc<dyn Send + Sync>> {
        self.0.claim(bytes).map(|lease| Arc::new(lease) as Arc<dyn Send + Sync>)
    }
    /// A zero limit deliberately rejects every allocation.
    pub fn new(max_bytes: u64) -> Self {
        Self(budget::Budget::new(max_bytes))
    }
    /// Known admitted GPU storage still retained by producers, images or in-flight readers.
    pub fn retained_bytes(&self) -> u64 {
        self.0.used()
    }
}

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
}
/// One coherent nonblocking renderer observation. None image explicitly withdraws pixels.
pub(super) struct Sample {
    pub sequence: u64,
    pub image: Option<GpuImage>,
}

/// Fixed-descriptor latest GPU image source. Writers cannot accumulate an image queue.
#[derive(Clone)]
pub struct GpuFrameStream {
    descriptor: TextureDescriptor,
    adapter: AdapterId,
    state: Arc<Mutex<State>>,
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
            descriptor,
            adapter,
            state: Default::default(),
        }
    }
    /// Immutable sampled RGB contract installed with a plan.
    pub fn descriptor(&self) -> TextureDescriptor {
        self.descriptor
    }
    /// Adapter required by every source image and renderer import.
    pub fn adapter(&self) -> AdapterId {
        self.adapter
    }
    /// Publish only a completed image, or withdraw. Dispose displaced storage outside locks.
    pub fn publish(&self, image: Option<GpuImage>) -> Result<(), &'static str> {
        self.replace(image).map(drop)
    }
    /// Replace under an owner's epoch fence, returning old handle/storage ownership so it
    /// can be disposed after that outer control lock is released.
    pub fn replace(&self, image: Option<GpuImage>) -> Result<Option<GpuImage>, &'static str> {
        if image
            .as_ref()
            .is_some_and(|image| image.descriptor() != self.descriptor || image.adapter != self.adapter)
        {
            return Err("GPU stream image contract changed");
        }
        let mut state = self.state.lock().map_err(|_| "GPU image stream poisoned")?;
        let sequence = state.sequence.checked_add(1).ok_or("GPU stream sequence exhausted")?;
        let old = std::mem::replace(&mut state.image, image);
        state.sequence = sequence;
        drop(state);
        Ok(old)
    }
    pub(super) fn sample(&self) -> Option<Sample> {
        let state = self.state.try_lock().ok()?;
        Some(Sample {
            sequence: state.sequence,
            image: state.image.clone(),
        })
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        size_of::<State>() + 2 * size_of::<usize>()
    }
}

#[cfg(test)]
pub(in crate::native) mod tests;
