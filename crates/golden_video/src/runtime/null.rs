use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

use golden_io::WorkerNotifier;
use golden_time::TimeSnapshot;

use crate::plan::CompiledPlan;

use super::backend::{Backend, BackendError};
use super::identity::{CompletionToken, DeviceGeneration};

/// Qualification backend settings. No pixel buffers or native interfaces are created.
#[derive(Clone, Copy, Debug)]
pub struct NullConfig {
    /// Complete successful simulated submissions immediately.
    pub auto_complete: bool,
    /// Extra simulated allocation charge for each distinct texture.
    pub padding_per_texture: u64,
    /// Cancellable CPU-preparation delay applied by the preparation worker, not render service.
    pub preparation_delay: Duration,
}

impl Default for NullConfig {
    fn default() -> Self {
        Self {
            auto_complete: true,
            padding_per_texture: 0,
            preparation_delay: Duration::ZERO,
        }
    }
}

/// Bounded metadata observations for the null qualification backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullSnapshot {
    /// Device bound to this control; absent before backend startup.
    pub device: Option<DeviceGeneration>,
    /// Number of simulated texture objects constructed over this lifetime.
    pub created: u64,
    /// Number of simulated texture objects destroyed over this lifetime.
    pub dropped: u64,
    /// Simulated texture objects currently alive.
    pub live: usize,
    /// Simulated bytes represented by live objects; not process/GPU memory usage.
    pub live_bytes: u64,
    /// Highest simulated live-byte total.
    pub peak_live_bytes: u64,
    /// Thread that most recently destroyed a fake resource.
    pub last_drop_thread: Option<ThreadId>,
    /// Last successfully submitted device-local serial.
    pub last_submitted: Option<CompletionToken>,
    /// Last manually/automatically completed serial.
    pub completed: Option<CompletionToken>,
    /// Shared authority observation used for the last successful submission.
    pub last_submission_time: Option<TimeSnapshot>,
    /// Explicit injected device-loss state.
    pub device_lost: bool,
    /// Valid device-reset attempts, including explicitly injected allocation failures.
    pub reset_attempts: u64,
}

#[derive(Default)]
struct State {
    snapshot: NullSnapshot,
    fail_prepare_after: Option<usize>,
    fail_submit: bool,
    fail_resets: usize,
    changed: u64,
    waker: Option<WorkerNotifier>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

/// Metadata-only manual completion/failure control for deterministic qualification.
#[derive(Clone, Default)]
pub struct NullControl(Arc<Shared>);

impl NullControl {
    /// Creates an unbound control; the worker-local backend binds its device once.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies bounded diagnostics without transferring resource ownership.
    pub fn snapshot(&self) -> NullSnapshot {
        self.0.state.lock().unwrap_or_else(|p| p.into_inner()).snapshot
    }

    /// Observes control changes through an existing nonblocking worker notification.
    pub(crate) fn set_waker(&self, waker: WorkerNotifier) {
        self.0.state.lock().unwrap_or_else(|p| p.into_inner()).waker = Some(waker);
    }

    /// Advances the current device's simulated completion. Foreign/older tokens are ignored.
    /// Future serials are permitted as fault injection and must be rejected by the core.
    pub fn complete_through(&self, token: CompletionToken) -> bool {
        let waker = {
            let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.snapshot.device != Some(token.device)
                || state
                    .snapshot
                    .completed
                    .is_some_and(|completed| token.serial < completed.serial)
            {
                return false;
            }
            state.snapshot.completed = Some(token);
            state.changed = state.changed.wrapping_add(1);
            state.waker.clone()
        };
        self.notify(waker);
        true
    }

    /// Fails the next preparation after this many successful fake allocations.
    pub fn fail_next_prepare_after(&self, allocations: usize) {
        let waker = {
            let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.fail_prepare_after = Some(allocations);
            state.changed = state.changed.wrapping_add(1);
            state.waker.clone()
        };
        self.notify(waker);
    }

    /// Rejects the next submission before it obtains a resource use/completion token.
    pub fn fail_next_submit(&self) {
        let waker = {
            let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.fail_submit = true;
            state.changed = state.changed.wrapping_add(1);
            state.waker.clone()
        };
        self.notify(waker);
    }

    /// Confirms simulated device loss; only reset can make this device usable again.
    pub fn lose_device(&self) {
        let waker = {
            let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.snapshot.device_lost = true;
            state.changed = state.changed.wrapping_add(1);
            state.waker.clone()
        };
        self.notify(waker);
    }

    /// Fails this many upcoming reset allocations while preserving simulated device loss.
    /// This tests retry/backoff and cancellation; it is not native hardware-removal evidence.
    pub fn fail_next_resets(&self, attempts: usize) {
        let waker = {
            let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.fail_resets = attempts;
            state.changed = state.changed.wrapping_add(1);
            state.waker.clone()
        };
        self.notify(waker);
    }

    fn notify(&self, waker: Option<WorkerNotifier>) {
        self.0.changed.notify_all();
        if let Some(waker) = waker {
            waker.notify();
        }
    }
}

struct NullTexture {
    shared: Arc<Shared>,
    bytes: u64,
    _worker_local: Rc<()>,
}

impl Drop for NullTexture {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.snapshot.dropped = state.snapshot.dropped.saturating_add(1);
        state.snapshot.live -= 1;
        state.snapshot.live_bytes -= self.bytes;
        state.snapshot.last_drop_thread = Some(thread::current().id());
    }
}

pub(super) struct NullResources {
    _textures: Vec<NullTexture>,
}

pub(super) struct NullBackend {
    config: NullConfig,
    device: DeviceGeneration,
    control: NullControl,
    _worker_local: Rc<()>,
}

impl NullBackend {
    pub(super) fn new(
        config: NullConfig,
        device: DeviceGeneration,
        control: NullControl,
    ) -> Result<Self, BackendError> {
        {
            let mut state = control.0.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.snapshot.device.is_some() {
                return Err(BackendError::AlreadyBound);
            }
            state.snapshot.device = Some(device);
            state.snapshot.completed = Some(CompletionToken { device, serial: 0 });
            state.snapshot.last_submitted = Some(CompletionToken { device, serial: 0 });
        }
        Ok(Self {
            config,
            device,
            control,
            _worker_local: Rc::default(),
        })
    }

    fn check_device(&self) -> Result<(), BackendError> {
        let snapshot = self.control.snapshot();
        if snapshot.device_lost || snapshot.device != Some(self.device) {
            Err(BackendError::DeviceLost)
        } else {
            Ok(())
        }
    }
}

impl Backend for NullBackend {
    type Resources = NullResources;

    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError> {
        self.check_device()?;
        let count = plan
            .external_textures()
            .len()
            .checked_add(plan.passes().len())
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(BackendError::ResourceSizeOverflow)?;
        self.config
            .padding_per_texture
            .checked_mul(count)
            .and_then(|padding| plan.footprint().total_texture_bytes.checked_add(padding))
            .ok_or(BackendError::ResourceSizeOverflow)
    }

    fn prepare(&mut self, plan: &CompiledPlan, device: DeviceGeneration) -> Result<Self::Resources, BackendError> {
        self.check_device()?;
        if device != self.device {
            return Err(BackendError::DeviceLost);
        }
        let fail_after = self
            .control
            .0
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .fail_prepare_after
            .take();
        let mut textures = Vec::with_capacity(plan.external_textures().len() + plan.passes().len());
        let descriptors = plan
            .external_textures()
            .iter()
            .map(|texture| texture.descriptor)
            .chain(plan.passes().iter().map(|pass| pass.descriptor));
        for descriptor in descriptors {
            if fail_after == Some(textures.len()) {
                return Err(BackendError::AllocationFailed);
            }
            let bytes = descriptor
                .byte_len()
                .ok()
                .and_then(|bytes| bytes.checked_add(self.config.padding_per_texture))
                .ok_or(BackendError::ResourceSizeOverflow)?;
            {
                let mut state = self.control.0.state.lock().unwrap_or_else(|p| p.into_inner());
                if state.snapshot.device_lost {
                    return Err(BackendError::DeviceLost);
                }
                state.snapshot.created = state.snapshot.created.saturating_add(1);
                state.snapshot.live += 1;
                state.snapshot.live_bytes += bytes;
                state.snapshot.peak_live_bytes = state.snapshot.peak_live_bytes.max(state.snapshot.live_bytes);
            }
            textures.push(NullTexture {
                shared: self.control.0.clone(),
                bytes,
                _worker_local: self._worker_local.clone(),
            });
        }
        if fail_after == Some(textures.len()) {
            return Err(BackendError::AllocationFailed);
        }
        Ok(NullResources { _textures: textures })
    }

    fn submit(
        &mut self,
        _resources: &Self::Resources,
        _plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        self.check_device()?;
        let mut state = self.control.0.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.snapshot.device_lost {
            return Err(BackendError::DeviceLost);
        }
        if state.fail_submit {
            state.fail_submit = false;
            return Err(BackendError::SubmissionFailed);
        }
        state.snapshot.last_submitted = Some(token);
        state.snapshot.last_submission_time = Some(time);
        if self.config.auto_complete {
            state.snapshot.completed = Some(token);
        }
        Ok(())
    }

    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        self.check_device()?;
        Ok(self.control.snapshot().completed.expect("bound null device"))
    }

    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        self.check_device()?;
        let state = self.control.0.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.snapshot.device_lost {
            return Err(BackendError::DeviceLost);
        }
        let observed = state.changed;
        if state
            .snapshot
            .completed
            .zip(state.snapshot.last_submitted)
            .is_some_and(|(completed, submitted)| completed.serial >= submitted.serial)
        {
            return Ok(());
        }
        let (state, _) = self
            .control
            .0
            .changed
            .wait_timeout_while(state, timeout, |state| {
                state.changed == observed && !state.snapshot.device_lost
            })
            .unwrap_or_else(|p| p.into_inner());
        if state.snapshot.device_lost {
            Err(BackendError::DeviceLost)
        } else {
            Ok(())
        }
    }

    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        let mut state = self.control.0.state.lock().unwrap_or_else(|p| p.into_inner());
        if !state.snapshot.device_lost {
            return Err(BackendError::DeviceLost);
        }
        state.snapshot.reset_attempts = state.snapshot.reset_attempts.saturating_add(1);
        if state.fail_resets != 0 {
            state.fail_resets -= 1;
            return Err(BackendError::AllocationFailed);
        }
        self.device = device;
        state.snapshot.device = Some(device);
        state.snapshot.device_lost = false;
        state.snapshot.completed = Some(CompletionToken { device, serial: 0 });
        state.snapshot.last_submitted = Some(CompletionToken { device, serial: 0 });
        state.snapshot.last_submission_time = None;
        state.changed = state.changed.wrapping_add(1);
        self.control.0.changed.notify_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
