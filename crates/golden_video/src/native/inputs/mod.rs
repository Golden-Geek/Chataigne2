//! Fixed-size source pools. SDK/device work stays on one worker per configured input;
//! renderer imports are local and remain leased through matching renderer completion.

mod reader;
mod receiver;
mod shared;
mod surface;
mod worker;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use golden_io::{BoundedWorker, RetirementPool, WorkerLimits, WorkerNotifier};
use windows::core::{Error, HRESULT, Result};

use crate::plan::CompiledPlan;
use crate::{TextureDescriptor, TextureId};

use super::device::Device;
use super::{AdapterId, D3d11Control, SpoutInput};
use reader::{Imported, Reader};
use shared::Shared;

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
    shared: Arc<Shared>,
    worker: BoundedWorker<(), ()>,
    imports: [Option<std::rc::Rc<Imported>>; 3],
}

pub(super) struct SourcePool {
    inputs: BTreeMap<TextureId, Input>,
    readers: Vec<(u64, TextureId, Reader)>,
    lifetimes: RetirementPool,
}

pub(super) struct SourceDemands {
    _leases: Vec<shared::Demand>,
}

impl SourcePool {
    pub fn new(
        adapter: AdapterId,
        bindings: &[SpoutInput],
        control: &D3d11Control,
        notify: Option<WorkerNotifier>,
        device: crate::runtime::DeviceGeneration,
    ) -> Result<Self> {
        let lifetimes = RetirementPool::new(NonZeroUsize::new(bindings.len().max(1)).unwrap());
        let mut pool = Self {
            inputs: BTreeMap::new(),
            readers: Vec::with_capacity(bindings.len() * 2),
            lifetimes,
        };
        let mut statuses = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let status = Arc::new(Mutex::new(SourceSnapshot {
                device,
                texture: binding.texture,
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
            statuses.push(status.clone());
            let shared = Arc::new(Shared {
                latest: Mutex::new(None),
                status,
                active: AtomicBool::new(false),
                retiring: AtomicBool::new(false),
                demands: AtomicUsize::new(0),
                quiescing: AtomicBool::new(false),
                released: AtomicBool::new(false),
                unlock_failed: AtomicBool::new(false),
                shutdown: AtomicBool::new(false),
                notify: notify.clone(),
                worker_notify: Mutex::new(None),
            });
            let worker_shared = shared.clone();
            let config = binding.clone();
            let worker = BoundedWorker::spawn(
                "golden-video-source",
                &pool.lifetimes,
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
            pool.inputs.insert(
                binding.texture,
                Input {
                    shared,
                    worker,
                    imports: [None, None, None],
                },
            );
        }
        control.register_sources(statuses);
        Ok(pool)
    }

    pub fn demands(&self, plan: &CompiledPlan) -> SourceDemands {
        SourceDemands {
            _leases: plan
                .external_textures()
                .iter()
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
        previous: u64,
    ) -> Result<Option<u64>> {
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
        if publication.sequence == previous {
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
        Ok(Some(publication.sequence))
    }
}

impl Drop for SourcePool {
    fn drop(&mut self) {
        // The owning core has drained renderer uses or confirmed renderer-device removal.
        // Release leases/imports/mailboxes before cancellation; no worker may allocate its
        // replacement pool until these aliases are acknowledged as released.
        self.readers.clear();
        for input in self.inputs.values_mut() {
            input.shared.quiesce();
            input.imports = [None, None, None];
            input.shared.clear();
            input.shared.shutdown.store(true, Ordering::Release);
            input.shared.released.store(true, Ordering::Release);
            input.worker.cancel();
        }
        while !self.lifetimes.wait_for_idle(Duration::from_millis(20)) {}
    }
}

#[cfg(test)]
mod tests;
