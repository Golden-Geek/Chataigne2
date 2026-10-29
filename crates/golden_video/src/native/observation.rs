use std::sync::{Arc, Mutex};

use crate::OutputId;
use crate::runtime::{CompletionToken, DeviceGeneration};

use super::{AdapterId, outputs::Delivery};

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
    /// Saturating count of actual baseline initialization attempts, including failures.
    pub initialization_attempts: u64,
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

/// One fixed native consumer's last API-accepted submission. This is neither scanout nor a GPU
/// completion certificate; consumers must check the exact device and required frame serial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputSnapshot {
    /// Logical consumer from the immutable native output configuration.
    pub id: OutputId,
    /// Most recent batch accepted by this consumer's API in its current device lifetime.
    pub last_accepted: Option<CompletionToken>,
    /// Endpoint availability/recovery, separate from the owning device's runtime state.
    pub state: OutputState,
    /// Successful endpoint incarnation within this device; zero before first construction.
    pub generation: u64,
    /// Saturating count of construction attempts, including failures.
    pub attempts: u64,
    /// Latest endpoint HRESULT; cleared by successful creation or delivery.
    pub hresult: Option<i32>,
}

impl OutputSnapshot {
    pub(super) fn starting(id: OutputId) -> Self {
        Self {
            id,
            last_accepted: None,
            state: OutputState::Starting,
            generation: 0,
            attempts: 0,
            hresult: None,
        }
    }
}

pub(super) struct OutputStatus {
    pub state: OutputState,
    pub generation: u64,
    pub attempts: u64,
    pub hresult: Option<i32>,
}

/// Per-consumer delivery and recovery lifecycle. Busy/occluded endpoints retain their objects;
/// ordinary failures recover independently from the renderer's device lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputState {
    /// Fixed binding exists but no native endpoint has been constructed.
    Starting,
    /// Native endpoint is available for delivery; acceptance is tracked separately.
    Ready,
    /// Presentation capacity is temporarily busy; no recreation is required.
    Busy,
    /// Endpoint is hidden/occluded or otherwise temporarily unavailable.
    Unavailable,
    /// Failed endpoint is awaiting its last-use completion or a capped retry deadline.
    Recovering,
    /// New delivery/retry admissions stopped; native resources can still be retiring.
    Stopping,
    /// Endpoint cleanup completed.
    Stopped,
    /// The owning device itself reports removal; endpoint recreation alone cannot repair it.
    DeviceLost,
}

#[derive(Default)]
struct OutputRegistry {
    device: Option<DeviceGeneration>,
    entries: Vec<OutputSnapshot>,
}

/// Metadata-only native observation reader. It never retains devices, contexts or textures.
#[derive(Clone, Default)]
pub struct D3d11Control {
    #[cfg(test)]
    pub(super) initialization_gate: Arc<Mutex<Option<super::tests::initialization_gate::InitializationGate>>>,
    pub(super) source_notifier: Arc<Mutex<Option<golden_io::WorkerNotifier>>>,
    output_notifier: Arc<Mutex<Option<golden_io::WorkerNotifier>>>,
    state: Arc<Mutex<D3d11Snapshot>>,
    sources: Arc<Mutex<Vec<SourceRegistration>>>,
    outputs: Arc<Mutex<OutputRegistry>>,
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
    /// Registers a metadata-only wake for output state/incarnation/error changes. Repeated
    /// frame acceptance and counter changes do not trigger notifications.
    pub fn set_output_notifier(&self, notifier: golden_io::WorkerNotifier) {
        *self.output_notifier.lock().unwrap_or_else(|error| error.into_inner()) = Some(notifier);
    }

    fn notify_output(&self) {
        let notifier = self
            .output_notifier
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(notifier) = notifier {
            notifier.notify();
        }
    }
    /// Copies bounded native status without joining or polling GPU work.
    pub fn snapshot(&self) -> D3d11Snapshot {
        *self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Copies metadata for the fixed configured consumers, without GPU polling or native
    /// resources. Busy, unavailable and failed deliveries never advance these acceptance tokens.
    pub fn output_snapshots(&self) -> Vec<OutputSnapshot> {
        self.outputs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .clone()
    }

    pub(super) fn register_outputs(&self, device: DeviceGeneration, outputs: impl Iterator<Item = OutputId>) {
        *self.outputs.lock().unwrap_or_else(|error| error.into_inner()) = OutputRegistry {
            device: Some(device),
            entries: outputs.map(OutputSnapshot::starting).collect(),
        };
        self.notify_output();
    }

    pub(super) fn clear_outputs(&self) {
        let mut outputs = self.outputs.lock().unwrap_or_else(|error| error.into_inner());
        let changed = outputs.device.is_some() || !outputs.entries.is_empty();
        outputs.device = None;
        outputs.entries.clear();
        drop(outputs);
        if changed {
            self.notify_output();
        }
    }

    pub(super) fn observe_output(&self, id: OutputId, token: CompletionToken, delivery: Delivery) {
        self.update(|state| match delivery {
            Delivery::Accepted => state.accepted_outputs = state.accepted_outputs.saturating_add(1),
            Delivery::Busy => state.busy_outputs = state.busy_outputs.saturating_add(1),
            Delivery::Unavailable => state.unavailable_outputs = state.unavailable_outputs.saturating_add(1),
            Delivery::Failed(error) | Delivery::DeviceLost(error) => {
                state.failed_outputs = state.failed_outputs.saturating_add(1);
                state.failure = Some(NativeFailure::Output);
                state.hresult = Some(error.0);
            }
        });
        if delivery == Delivery::Accepted {
            let mut outputs = self.outputs.lock().unwrap_or_else(|error| error.into_inner());
            let mut first_acceptance = false;
            if outputs.device == Some(token.device)
                && let Some(output) = outputs.entries.iter_mut().find(|output| output.id == id)
            {
                first_acceptance = output.last_accepted.is_none();
                output.last_accepted = Some(token);
            }
            drop(outputs);
            if first_acceptance {
                self.notify_output();
            }
        }
    }

    pub(super) fn output_state(&self, id: OutputId, device: DeviceGeneration, status: OutputStatus) {
        let mut outputs = self.outputs.lock().unwrap_or_else(|error| error.into_inner());
        if outputs.device != Some(device) {
            return;
        }
        let OutputStatus {
            state,
            generation,
            attempts,
            hresult,
        } = status;
        let mut changed = false;
        if let Some(output) = outputs.entries.iter_mut().find(|output| output.id == id) {
            // Ready/Busy can alternate every frame under presentation backpressure. Once an
            // incarnation has accepted work, that pair has the same observed availability.
            let backpressure = output.last_accepted.is_some()
                && matches!(output.state, OutputState::Ready | OutputState::Busy)
                && matches!(state, OutputState::Ready | OutputState::Busy);
            changed = (!backpressure && output.state != state)
                || output.generation != generation
                || output.hresult != hresult;
            if generation != output.generation
                || matches!(
                    state,
                    OutputState::Recovering | OutputState::Stopping | OutputState::Stopped | OutputState::DeviceLost
                )
            {
                output.last_accepted = None;
            }
            output.state = state;
            output.generation = generation;
            output.attempts = attempts;
            output.hresult = hresult;
        }
        drop(outputs);
        if changed {
            self.notify_output();
        }
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

#[cfg(test)]
mod tests;
