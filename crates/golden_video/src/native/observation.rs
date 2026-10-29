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
    /// Immutable CPU images supplied as initial data at native texture creation, never per frame.
    pub uploaded_images: u64,
    /// Tight source texel bytes supplied through those one-time initial image uploads.
    pub uploaded_image_bytes: u64,
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
    pub(super) source_notifier: Arc<Mutex<Option<golden_io::WorkerNotifier>>>,
    state: Arc<Mutex<D3d11Snapshot>>,
    sources: Arc<Mutex<Vec<SourceRegistration>>>,
}

#[derive(Clone)]
pub(super) struct SourceRegistration {
    pub state: Arc<Mutex<super::SourceSnapshot>>,
    pub revision: golden_io::RevisionReader,
}

/// Coherent metadata and a fence for its source layout/session/pool lifetime.
#[derive(Clone, Debug)]
pub struct SourceObservationSnapshot {
    /// Captured source metadata.
    pub snapshot: super::SourceSnapshot,
    /// Rechecked by guarded plan preparation and installation.
    pub guard: golden_io::RevisionGuard,
}

impl D3d11Control {
    /// Registers a preparation-worker wake for source state/layout/lifetime changes.
    /// Frame/publication counters do not trigger this notification.
    pub fn set_source_notifier(&self, notifier: golden_io::WorkerNotifier) {
        *self.source_notifier.lock().unwrap_or_else(|error| error.into_inner()) = Some(notifier);
    }
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
            .map(|source| *source.state.lock().unwrap_or_else(|error| error.into_inner()))
            .collect()
    }

    /// Captures source data and its layout revision under the same metadata lock.
    pub fn source_observations(&self) -> Vec<SourceObservationSnapshot> {
        let sources = self.sources.lock().unwrap_or_else(|error| error.into_inner()).clone();
        sources
            .iter()
            .map(|source| {
                let snapshot = source.state.lock().unwrap_or_else(|error| error.into_inner());
                SourceObservationSnapshot {
                    snapshot: *snapshot,
                    guard: source.revision.guard_at(source.revision.current()),
                }
            })
            .collect()
    }

    pub(super) fn register_sources(&self, sources: Vec<SourceRegistration>) {
        let mut current = self.sources.lock().unwrap_or_else(|error| error.into_inner());
        let changed = current.len() != sources.len()
            || current
                .iter()
                .zip(&sources)
                .any(|(a, b)| !Arc::ptr_eq(&a.state, &b.state));
        *current = sources;
        drop(current);
        if changed && let Some(notifier) = &*self.source_notifier.lock().unwrap_or_else(|error| error.into_inner()) {
            notifier.notify();
        }
    }
}
