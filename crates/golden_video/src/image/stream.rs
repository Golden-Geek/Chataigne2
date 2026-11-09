//! Fixed-descriptor latest-image publication. Render consumers never wait for producers.
use super::ImageData;
use crate::TextureDescriptor;
use crate::scheduling::FrameWindow;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default)]
struct State {
    sequence: u64,
    image: Option<Arc<ImageData>>,
    frame: Option<FramePublication>,
}

/// Metadata committed atomically with one decoded image, not its later upload.
#[derive(Clone, Copy, Debug)]
pub struct FramePublication {
    /// Producer-declared PTS and presentation interval; this latest-image slot does not schedule it.
    pub window: FrameWindow,
    /// Monotonic observation immediately before the image becomes visible to consumers.
    pub published_at: Instant,
}

/// A bounded latest-image source shared by decoded-frame producers and render generations.
/// Pixel storage is admitted by the producer; this slot retains at most one image.
#[derive(Clone)]
pub struct ImageStream {
    id: uuid::Uuid,
    descriptor: TextureDescriptor,
    state: Arc<Mutex<State>>,
}
impl std::fmt::Debug for ImageStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageStream")
            .field("descriptor", &self.descriptor)
            .finish_non_exhaustive()
    }
}
/// One coherent observation. None image means withdraw the previous pixels.
pub struct StreamSample {
    /// Monotonic publication identity, including withdrawals.
    pub sequence: u64,
    /// Immutable tightly packed software image, owned independently of its decoder.
    pub image: Option<Arc<ImageData>>,
    /// Exact decoded-frame metadata; absent for ordinary images and withdrawals.
    pub frame: Option<FramePublication>,
}
impl ImageStream {
    /// Create a fixed-descriptor source. Extent changes require a newly admitted stream.
    pub fn new(descriptor: TextureDescriptor) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            descriptor,
            state: Default::default(),
        }
    }
    /// The immutable storage/color contract installed with a render plan.
    pub fn descriptor(&self) -> TextureDescriptor {
        self.descriptor
    }

    /// Opaque identity of this publication lifetime, shared by cloned consumers.
    pub fn id(&self) -> uuid::Uuid {
        self.id
    }
    /// Publish or withdraw one image on the producer worker. No queue can accumulate.
    pub fn publish(&self, image: Option<ImageData>) -> Result<(), &'static str> {
        self.replace(image).map(drop)
    }
    /// Replace while retaining the displaced image for disposal outside an enclosing
    /// control lock. Publication fences must not keep an engine caller waiting on pixel frees.
    pub fn replace(&self, image: Option<ImageData>) -> Result<Option<Arc<ImageData>>, &'static str> {
        self.replace_inner(image, None)
    }
    /// Publish decoded pixels and their actual timestamp under the same sequence/fence.
    /// Return displaced storage for disposal outside the producer's control lock.
    pub fn replace_frame(&self, image: ImageData, window: FrameWindow) -> Result<Option<Arc<ImageData>>, &'static str> {
        self.replace_inner(Some(image), Some(window))
    }
    fn replace_inner(
        &self,
        image: Option<ImageData>,
        window: Option<FrameWindow>,
    ) -> Result<Option<Arc<ImageData>>, &'static str> {
        if image
            .as_ref()
            .is_some_and(|image| image.descriptor() != self.descriptor)
        {
            return Err("stream image descriptor changed");
        }
        let mut state = self.state.lock().map_err(|_| "image stream poisoned")?;
        let sequence = state.sequence.checked_add(1).ok_or("image stream sequence exhausted")?;
        let frame = window.map(|window| FramePublication {
            window,
            published_at: Instant::now(),
        });
        let old = std::mem::replace(&mut state.image, image.map(Arc::new));
        state.frame = frame;
        state.sequence = sequence;
        drop(state);
        Ok(old)
    }
    /// Render-worker observation. None means contention: retain the last uploaded pixels.
    pub fn sample(&self) -> Option<StreamSample> {
        let state = self.state.try_lock().ok()?;
        Some(StreamSample {
            sequence: state.sequence,
            image: state.image.clone(),
            frame: state.frame,
        })
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<State>() + 2 * std::mem::size_of::<usize>()
    }
}
