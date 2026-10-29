//! Fixed-size source pools. SDK/device work stays on one worker per configured input;
//! renderer imports are local and remain leased through matching renderer completion.

mod reader;
pub(crate) mod receiver;
mod shared;
mod surface;
mod worker;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use golden_io::{BoundedWorker, PendingDrainState, RetirementPool, WorkerLimits, WorkerNotifier};
use windows::core::{Error, HRESULT, Result};

use crate::plan::{CompiledPlan, ExternalSource, SpoutSource};
use crate::runtime::backend::BackendError;
use crate::{TextureDescriptor, TextureId};

use super::device::Device;
use super::observation::SourceRegistration;
use super::{AdapterId, D3d11Control, InputLimits};
use reader::{Imported, Reader};
use shared::Shared;

/// Exact contents retained by one renderer target, including unchanged/busy source reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CopyFrame {
    pub sequence: u64,
    pub timing: Option<super::PublicationTiming>,
}

/// Source state is independent of other inputs and of the renderer's device state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceState {
    /// No prepared/live/retiring generation retains demand; no SDK receive polling occurs.
    Idle,
    /// The exact sender is absent or has not provided an SDK-reported frame yet.
    WaitingForSender,
    /// Sender storage/extent differs from the admitted immutable descriptor.
    DescriptorMismatch,
    /// SDK-reported frames have been relayed to the bounded GPU bridge.
    /// As with SDK sends, this observation is not a delivery/cadence proof.
    Streaming,
    /// Its device/bridge is retiring before a bounded retry; healthy inputs keep running.
    Recovering,
    /// Source worker has finished native cleanup.
    Stopped,
}

/// Bounded metadata only; no source device, texture or shared handle leaves the native layer.
#[derive(Clone, Copy, Debug)]
pub struct SourceSnapshot {
    /// Exact subscription lifetime, including re-creation on the same renderer device.
    pub binding: uuid::Uuid,
    /// Renderer device lifetime owning this binding and its reservation.
    pub device: crate::runtime::DeviceGeneration,
    /// Logical external texture binding.
    pub texture: TextureId,
    /// Admitted maximum extent and required storage/color/alpha layout.
    pub descriptor: TextureDescriptor,
    /// Most recently accepted actual extent/storage. Color/alpha remain caller-declared.
    pub observed_descriptor: Option<TextureDescriptor>,
    /// Current source-local lifecycle.
    pub state: SourceState,
    /// Successful source-device sessions in this renderer lifetime.
    pub generation: u64,
    /// Successfully created capture/bridge pools, including bounded size changes.
    pub pool_generation: u64,
    /// Owned bridge slots currently allocated, at most three.
    pub bridge_slots: usize,
    /// SDK receive calls, performed only on this source's worker.
    pub sdk_calls: u64,
    /// SDK-reported frames handed to the GPU bridge, not proved fresh source deliveries.
    pub publications: u64,
    /// The latest runtime-local publication sequence; never reused after reconnect.
    pub sequence: u64,
    /// Source polls skipped because all three admitted slots were retained.
    pub bridge_busy: u64,
    /// Renderer-local shared-resource opens (cached by allocation identity).
    pub imports: u64,
    /// Copies into generation-owned renderer textures.
    pub renderer_copies: u64,
    /// Last endpoint/device error, when supplied by an API.
    pub hresult: Option<i32>,
}

struct Input {
    binding: SpoutSource,
    stopping: bool,
    shared: Arc<Shared>,
    worker: BoundedWorker<(), ()>,
    imports: [Option<std::rc::Rc<Imported>>; 3],
}

pub(super) struct SourcePool {
    inputs: BTreeMap<TextureId, Input>,
    readers: Vec<(u64, TextureId, Reader)>,
    lifetimes: RetirementPool,
    adapter: AdapterId,
    limits: InputLimits,
    control: D3d11Control,
    notify: Option<WorkerNotifier>,
    device: crate::runtime::DeviceGeneration,
}

pub(super) struct SourceDemands {
    _leases: Vec<shared::Demand>,
}

impl SourcePool {
    pub fn new(
        adapter: AdapterId,
        limits: InputLimits,
        control: &D3d11Control,
        notify: Option<WorkerNotifier>,
        device: crate::runtime::DeviceGeneration,
    ) -> Self {
        control.register_sources(Vec::new());
        let lifetimes = RetirementPool::new(NonZeroUsize::new(limits.max_sources.max(1)).unwrap());
        Self {
            inputs: BTreeMap::new(),
            readers: Vec::with_capacity(limits.max_sources * 2),
            lifetimes,
            adapter,
            limits,
            control: control.clone(),
            notify,
            device,
        }
    }

    fn spawn(&mut self, texture: TextureId, binding: SpoutSource) -> Result<()> {
        let device = self.device;
        let status = Arc::new(Mutex::new(SourceSnapshot {
            binding: uuid::Uuid::new_v4(),
            device,
            texture,
            descriptor: binding.descriptor,
            state: SourceState::Idle,
            generation: 0,
            pool_generation: 0,
            bridge_slots: 0,
            observed_descriptor: None,
            sdk_calls: 0,
            publications: 0,
            sequence: 0,
            bridge_busy: 0,
            imports: 0,
            renderer_copies: 0,
            hresult: None,
        }));
        let shared = Arc::new(Shared {
            diagnostics: self.control.recorder(),
            observation_revision: Default::default(),
            source_notifier: self.control.source_notifier.clone(),
            latest: Mutex::new(None),
            status,
            active: AtomicBool::new(false),
            retiring: AtomicBool::new(false),
            demands: AtomicUsize::new(0),
            quiescing: AtomicBool::new(false),
            released: AtomicBool::new(false),
            unlock_failed: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            notify: self.notify.clone(),
            worker_notify: Mutex::new(None),
        });
        let worker_shared = shared.clone();
        let config = binding.clone();
        let adapter = self.adapter;
        let worker = BoundedWorker::spawn(
            "golden-video-source",
            &self.lifetimes,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| worker::run(context, adapter, config, worker_shared),
        )
        .map_err(|error| Error::new(HRESULT(0x8000_4005_u32 as i32), error.to_string()))?;
        *shared.worker_notify.lock().unwrap_or_else(|error| error.into_inner()) = Some(worker.wake_notifier());
        self.inputs.insert(
            texture,
            Input {
                binding,
                stopping: false,
                shared,
                worker,
                imports: [None, None, None],
            },
        );
        Ok(())
    }

    fn stop(input: &mut Input) {
        input.stopping = true;
        input.shared.quiesce();
        input.imports = [None, None, None];
        input.shared.clear();
        input.shared.shutdown.store(true, Ordering::Release);
        input.shared.released.store(true, Ordering::Release);
        input.worker.cancel();
    }

    fn publish_statuses(&self) {
        self.control.register_sources(
            self.inputs
                .values()
                .map(|input| SourceRegistration {
                    state: input.shared.status.clone(),
                    revision: input.shared.observation_revision.reader(),
                })
                .collect(),
        );
    }

    pub fn ensure(&mut self, plan: &CompiledPlan) -> std::result::Result<(), BackendError> {
        // A binding may outlive its plan through GPU retirement. Only unused cache entries
        // can be evicted; cancellation never waits for SDK/GPU work on the renderer.
        for (id, input) in &mut self.inputs {
            if input.shared.demands.load(Ordering::Acquire) == 0
                && !self.readers.iter().any(|(_, source, _)| source == id)
                && !plan
                    .external_textures()
                    .iter()
                    .any(|source| source.id == *id && source.source.as_ref().and_then(ExternalSource::spout).is_some())
            {
                Self::stop(input);
            }
        }
        self.inputs.retain(|_, input| {
            !input.stopping
                || input.worker.drain_events(&mut Vec::new(), NonZeroUsize::MIN).state
                    != PendingDrainState::Disconnected
        });
        self.publish_statuses();
        let mut count = self.inputs.len();
        let mut bytes = self
            .inputs
            .values()
            .try_fold(0_u64, |sum, input| {
                sum.checked_add(input.binding.descriptor.byte_len().ok()?.checked_mul(5)?)
            })
            .ok_or(BackendError::ResourceSizeOverflow)?;
        for source in plan.external_textures() {
            let Some(binding) = source.source.as_ref().and_then(ExternalSource::spout) else {
                continue;
            };
            if let Some(input) = self.inputs.get(&source.id) {
                if input.binding != *binding {
                    return Err(BackendError::UnsupportedPlan);
                }
                if input.stopping {
                    return Err(BackendError::SourceCapacity);
                }
            } else {
                count += 1;
                bytes = bytes
                    .checked_add(
                        binding
                            .descriptor
                            .byte_len()
                            .map_err(|_| BackendError::ResourceSizeOverflow)?
                            .checked_mul(5)
                            .ok_or(BackendError::ResourceSizeOverflow)?,
                    )
                    .ok_or(BackendError::ResourceSizeOverflow)?;
            }
        }
        if count > self.limits.max_sources
            || bytes > self.limits.max_storage_bytes
            || self.lifetimes.metrics().active + count - self.inputs.len() > self.limits.max_sources
        {
            return Err(BackendError::SourceCapacity);
        }
        let mut created = Vec::new();
        for source in plan.external_textures() {
            let Some(binding) = source.source.as_ref().and_then(ExternalSource::spout) else {
                continue;
            };
            if !self.inputs.contains_key(&source.id) {
                if self.spawn(source.id, binding.clone()).is_err() {
                    for id in created {
                        Self::stop(self.inputs.get_mut(&id).unwrap());
                    }
                    self.publish_statuses();
                    return Err(BackendError::AllocationFailed);
                }
                created.push(source.id);
            }
        }
        self.publish_statuses();
        Ok(())
    }

    pub fn demands(&self, plan: &CompiledPlan) -> SourceDemands {
        SourceDemands {
            _leases: plan
                .external_textures()
                .iter()
                .filter(|source| source.source.as_ref().and_then(ExternalSource::spout).is_some())
                .map(|source| self.inputs[&source.id].shared.acquire_demand())
                .collect(),
        }
    }

    pub fn quiesce(&self) {
        for input in self.inputs.values() {
            input.shared.quiesce();
        }
    }

    pub fn retire(&mut self, completed: u64) {
        self.readers.retain(|(serial, _, _)| *serial > completed);
        for (id, input) in &mut self.inputs {
            if input.shared.retiring.load(Ordering::Acquire) && !self.readers.iter().any(|(_, source, _)| source == id)
            {
                input.imports = [None, None, None];
                input.shared.released.store(true, Ordering::Release);
            }
        }
    }

    pub fn fault(&self, id: TextureId, error: Error) {
        let shared = &self.inputs[&id].shared;
        shared.clear();
        shared.unlock_failed.store(true, Ordering::Release);
        shared.update(|status| status.hresult = Some(error.code().0));
        shared.state(SourceState::Recovering);
        if let Some(notify) = &*shared.worker_notify.lock().unwrap_or_else(|error| error.into_inner()) {
            notify.notify();
        }
    }

    /// Returns the admitted sequence, or None for unavailable input. The caller initializes missing input
    /// storage explicitly and tracks `sequence` separately for every prepared generation.
    pub fn copy(
        &mut self,
        id: TextureId,
        device: &Device,
        target: &super::texture::Texture,
        serial: u64,
        previous: CopyFrame,
    ) -> Result<Option<CopyFrame>> {
        let input = self.inputs.get_mut(&id).expect("validated input binding");
        if input.shared.retiring.load(Ordering::Acquire) {
            return Ok(None);
        }
        let publication = match input.shared.latest.try_lock() {
            Ok(latest) => latest.clone(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(Some(previous)),
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner().clone(),
        };
        let Some(publication) = publication else {
            return Ok(None);
        };
        if publication.surface.descriptor != target.descriptor {
            return Ok(None);
        }
        if publication.sequence == previous.sequence {
            return Ok(Some(previous));
        }
        let index = publication.surface.index;
        let imported = match &input.imports[index] {
            Some(imported) if imported.same_surface(&publication.surface) => imported.clone(),
            _ => {
                let imported = std::rc::Rc::new(Imported::open(device, &publication.surface)?);
                input.imports[index] = Some(imported.clone());
                input
                    .shared
                    .update(|status| status.imports = status.imports.saturating_add(1));
                imported
            }
        };
        let Some(reader) = Reader::acquire(imported, publication.surface, input.shared.clone())? else {
            return Ok(Some(previous));
        };
        // Lease before entering native CopyResource, including unwinding submission paths.
        self.readers.push((serial, id, reader));
        let reader = &self.readers.last().unwrap().2;
        // SAFETY: both descriptors were checked and imports belong to this renderer device.
        unsafe {
            device
                .context
                .CopyResource(&target.texture, &reader.imported.texture.texture);
        }
        input
            .shared
            .update(|status| status.renderer_copies = status.renderer_copies.saturating_add(1));
        Ok(Some(CopyFrame {
            sequence: publication.sequence,
            timing: publication.timing,
        }))
    }
}

impl Drop for SourcePool {
    fn drop(&mut self) {
        // The owning core has drained renderer uses or confirmed renderer-device removal.
        // Release leases/imports/mailboxes before cancellation; no worker may allocate its
        // replacement pool until these aliases are acknowledged as released.
        self.readers.clear();
        for input in self.inputs.values_mut() {
            Self::stop(input);
        }
        while !self.lifetimes.wait_for_idle(Duration::from_millis(20)) {}
    }
}

#[cfg(test)]
mod tests;
