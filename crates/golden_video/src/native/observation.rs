use std::sync::{Arc, Mutex};

use crate::runtime::DeviceGeneration;

use super::AdapterId;

/// Compact failure stage. No COM objects, driver strings or pixel payloads leave the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFailure {
    /// Native device/pipeline/output initialization failed.
    Initialization,
    /// External input is unbound or exceeds its declared storage/extent contract.
    UnsupportedExternalInput,
    /// A plan demanded a consumer that has no native binding.
    UnboundOutput,
    /// A native consumer failed after GPU work was queued; completion ownership is retained.
    Output,
}

/// Bounded aggregate observations, distinct from display/scanout or pixel correctness evidence.
#[derive(Clone, Copy, Debug, Default)]
pub struct D3d11Snapshot {
    /// Selected hardware adapter observed on the created device.
    pub adapter: Option<AdapterId>,
    /// Exact native device lifetime.
    pub device: Option<DeviceGeneration>,
    /// Whether initialized native resources currently exist.
    pub initialized: bool,
    /// GPU command batches with an established completion query.
    pub submitted: u64,
    /// Native output submissions accepted by their API, not proved displayed frames.
    pub accepted_outputs: u64,
    /// Outputs skipped without waiting because their presentation queue was busy.
    pub busy_outputs: u64,
    /// Outputs unavailable/occluded at submission.
    pub unavailable_outputs: u64,
    /// Output API failures; these never abandon already queued GPU uses.
    pub failed_outputs: u64,
    /// Latest compact native failure, if any.
    pub failure: Option<NativeFailure>,
    /// Last native HRESULT, when an API supplied one. This is metadata, not a driver payload.
    pub hresult: Option<i32>,
}

/// Metadata-only native observation reader. It never retains devices, contexts or textures.
#[derive(Clone, Default)]
pub struct D3d11Control {
    state: Arc<Mutex<D3d11Snapshot>>,
    sources: Arc<Mutex<Vec<Arc<Mutex<super::SourceSnapshot>>>>>,
}

impl D3d11Control {
    /// Copies bounded native status without joining or polling GPU work.
    pub fn snapshot(&self) -> D3d11Snapshot {
        *self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn update(&self, update: impl FnOnce(&mut D3d11Snapshot)) {
        update(&mut self.state.lock().unwrap_or_else(|error| error.into_inner()));
    }

    /// Bounded source metadata. This does not poll any native endpoint or retain GPU objects.
    pub fn sources(&self) -> Vec<super::SourceSnapshot> {
        let sources = self.sources.lock().unwrap_or_else(|error| error.into_inner()).clone();
        sources
            .iter()
            .map(|source| *source.lock().unwrap_or_else(|error| error.into_inner()))
            .collect()
    }

    pub(super) fn register_sources(&self, sources: Vec<Arc<Mutex<super::SourceSnapshot>>>) {
        *self.sources.lock().unwrap_or_else(|error| error.into_inner()) = sources;
    }
}
