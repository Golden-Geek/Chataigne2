//! Renderer-device aliases are bounded independently of producer reader ownership.

mod cache;
mod metrics;

use super::{
    Shared,
    surface::{self, Publication, Surface},
};
use crate::{ProbeResult, d3d::Gpu};
use cache::WeakCache;
pub use metrics::ImportReport;
use metrics::Metrics;
pub(super) use metrics::{Counts, Stage};
use std::{
    cell::Cell,
    os::windows::io::AsRawHandle,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Graphics::{Direct3D11::*, Dxgi::IDXGIKeyedMutex},
    },
    core::Interface,
};

#[derive(Default)]
struct Retention {
    live: Cell<usize>,
    peak: Cell<usize>,
}

struct AliasLease(Rc<Retention>);

impl AliasLease {
    fn new(retention: &Rc<Retention>) -> Self {
        let live = retention.live.get() + 1;
        retention.live.set(live);
        retention.peak.set(retention.peak.get().max(live));
        Self(retention.clone())
    }
}

impl Drop for AliasLease {
    fn drop(&mut self) {
        self.0.live.set(self.0.live.get() - 1);
    }
}

struct Imported {
    texture: ID3D11Texture2D,
    mutex: IDXGIKeyedMutex,
    // Declared last so the count retires after both COM aliases.
    _lease: AliasLease,
}

pub struct Frame {
    pub batch: u64,
    imported: Rc<Imported>,
    // Only a publication or an acquired reader owns this strong allocation identity.
    pub surface: Arc<Surface>,
    shared: Arc<Shared>,
}

impl Frame {
    pub fn texture(&self) -> &ID3D11Texture2D {
        &self.imported.texture
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        // SAFETY: only a successful key-1 acquisition constructs a Frame. Queued reads
        // precede producer reuse; the strong Surface remains until after local aliases.
        if unsafe { self.imported.mutex.ReleaseSync(0) }.is_err() {
            self.shared.unlock_failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(super) struct Imports {
    device: ID3D11Device1,
    cache: WeakCache<Surface, Rc<Imported>>,
    retention: Rc<Retention>,
    metrics: Metrics,
    peak_cached: usize,
}

impl Imports {
    pub(super) fn new(gpu: &Gpu, capacity: usize) -> ProbeResult<Self> {
        let mut metrics = Metrics::default();
        let start = Instant::now();
        let device = gpu.device.cast::<ID3D11Device1>();
        metrics.record(Stage::BindDevice, start, false);
        Ok(Self {
            device: device?,
            cache: WeakCache::new(capacity),
            retention: Rc::default(),
            metrics,
            peak_cached: 0,
        })
    }

    pub(super) fn open(
        &mut self,
        publication: Publication,
        shared: Arc<Shared>,
        collecting: bool,
    ) -> ProbeResult<Option<Frame>> {
        let Publication { surface, batch } = publication;
        let start = Instant::now();
        let pruned = self.cache.prune();
        self.record(Stage::Prune, start, collecting);
        self.count(collecting, |c| c.pruned += pruned as u64);
        let start = Instant::now();
        let cached = self.cache.get(&surface);
        self.record(Stage::Lookup, start, collecting);
        let imported = if let Some(imported) = cached {
            self.count(collecting, |c| c.cache_hits += 1);
            imported
        } else {
            self.count(collecting, |c| c.cache_misses += 1);
            let start = Instant::now();
            // SAFETY: this strong publication retains the process-owned NT handle. The
            // cache is bound once to the renderer device on the producer's adapter.
            let texture = unsafe {
                self.device
                    .OpenSharedResource1::<ID3D11Texture2D>(HANDLE(surface.handle.as_raw_handle()))
            };
            self.record(Stage::OpenResource, start, collecting);
            let texture = texture?;
            self.count(collecting, |c| c.opens += 1);
            let start = Instant::now();
            let mutex = texture.cast::<IDXGIKeyedMutex>();
            self.record(Stage::CastMutex, start, collecting);
            let imported = Rc::new(Imported {
                texture,
                mutex: mutex?,
                _lease: AliasLease::new(&self.retention),
            });
            let start = Instant::now();
            let evicted = self.cache.insert(&surface, imported.clone());
            let eviction = evicted.is_some();
            drop(evicted);
            self.record(Stage::Insert, start, collecting);
            self.count(collecting, |c| c.evictions += u64::from(eviction));
            self.peak_cached = self.peak_cached.max(self.cache.len());
            imported
        };
        let start = Instant::now();
        let acquired = surface::acquire(&imported.mutex, 1);
        self.record(Stage::AcquireKey, start, collecting);
        if !acquired? {
            // A cached alias owns neither a key nor a strong Surface. A timeout cannot
            // release the producer's key or displace the previous acquired reader.
            self.count(collecting, |c| c.acquire_timeouts += 1);
            return Ok(None);
        }
        Ok(Some(Frame {
            batch,
            imported,
            surface,
            shared,
        }))
    }

    pub(super) fn release_reader(&mut self, reader: Option<Frame>, collecting: bool) {
        if reader.is_some() {
            let start = Instant::now();
            drop(reader);
            self.record(Stage::ReleaseReader, start, collecting);
        }
    }

    pub(super) fn clear(&mut self, collecting: bool) {
        let start = Instant::now();
        let cleared = self.cache.clear();
        self.record(Stage::Clear, start, collecting);
        self.count(collecting, |c| c.cleared += cleared as u64);
    }

    pub(super) fn count(&mut self, collecting: bool, update: impl Fn(&mut Counts)) {
        self.metrics.count(collecting, update);
    }

    pub(super) fn record(&mut self, stage: Stage, start: Instant, collecting: bool) {
        self.metrics.record(stage, start, collecting);
    }

    #[cfg(test)]
    pub(super) fn counts(&self) -> &Counts {
        &self.metrics.all
    }

    #[cfg(test)]
    pub(super) fn cached(&self) -> usize {
        self.cache.len()
    }

    pub(super) fn finish(mut self) -> ImportReport {
        self.clear(false);
        self.metrics.finish(
            self.cache.capacity(),
            self.peak_cached,
            self.cache.len(),
            self.retention.live.get(),
            self.retention.peak.get(),
        )
    }
}
