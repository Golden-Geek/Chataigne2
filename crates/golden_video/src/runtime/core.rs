use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::time::Duration;

use golden_time::TimeSnapshot;
use thiserror::Error;

use crate::plan::CompiledPlan;

use super::backend::{Backend, BackendError};
use super::identity::{CompletionToken, DeviceGeneration, GenerationId, PlanIdentity};

/// Allocation and pending-use ceilings for one renderer.
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    /// Backend-declared resource charge across baseline/prepared/live/retiring pools.
    /// Native physical residency and opaque driver overhead require separate qualification.
    pub max_bytes: u64,
    /// At most two submitted batches may await completion.
    pub max_in_flight: NonZeroUsize,
    /// At most two replaced generations may await their final use.
    pub max_retiring: usize,
}

/// Metadata for one successfully prepared resource generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationInfo {
    /// Resource-set identity, separate from authored plan identity.
    pub generation: GenerationId,
    /// Desired-plan publication represented by this generation.
    pub identity: PlanIdentity,
    /// Exact device lifetime owning these resources.
    pub device: DeviceGeneration,
    /// Reserved full allocation charge.
    pub bytes: u64,
    /// Number of demanded output bindings.
    pub output_count: usize,
}

/// Successful submitted use of one resource generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmissionInfo {
    /// Resource set held through this use.
    pub generation: GenerationId,
    /// Desired plan used by the submission.
    pub identity: PlanIdentity,
    /// Device-scoped completion required before release.
    pub token: CompletionToken,
}

/// Worker-published resource/admission observations; contains no resource ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceSnapshot {
    /// Configured full-resource ceiling.
    pub byte_limit: u64,
    /// Currently reserved bytes, including baseline resources and prepared candidates.
    pub bytes_used: u64,
    /// Highest reserved byte total in this runtime lifetime.
    pub peak_bytes: u64,
    /// Outstanding prepared resource sets, bounded to one.
    pub prepared_generations: usize,
    /// Replaced resource sets still retained for GPU use.
    pub retiring_generations: usize,
    /// Submitted batches awaiting completion.
    pub in_flight: usize,
    /// Current installed resource set.
    pub active: Option<GenerationInfo>,
    /// Latest successfully submitted serial on this device.
    pub last_submitted: CompletionToken,
    /// Latest validated completion on this device.
    pub last_completed: CompletionToken,
    /// Whether first initialization succeeded. Remains true through later device-loss recovery;
    /// `device_lost` fences that separate stage. False never authorizes GPU use or completion.
    pub initialized: bool,
    /// Whether a backend operation confirmed device loss.
    pub device_lost: bool,
}

/// Worker-local admission or backend error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CoreError {
    /// Baseline admission succeeded but first backend initialization has not.
    #[error("render backend has not initialized")]
    NotInitialized,
    /// Captured authoring/prepared-asset state changed before resource admission.
    #[error("render plan source changed before preparation")]
    StaleSource,
    /// A configured bound violates the fixed renderer contract.
    #[error("invalid resource limits: {0}")]
    InvalidLimits(&'static str),
    /// A prepared generation already occupies the single candidate slot.
    #[error("a resource generation is already prepared")]
    PreparationBusy,
    /// Resource admission failed before backend allocation.
    #[error("resource charge {requested} with {used} already reserved exceeds {limit}")]
    ResourceBudgetExceeded {
        /// Requested full charge.
        requested: u64,
        /// Already retained charge.
        used: u64,
        /// Configured ceiling.
        limit: u64,
    },
    /// No submission slot is available without waiting.
    #[error("render in-flight capacity is exhausted")]
    InFlightFull,
    /// Replacing/clearing the live generation would exceed retained generations.
    #[error("render retirement capacity is exhausted")]
    RetirementFull,
    /// Runtime counters must never wrap to a previously valid identity/token.
    #[error("render generation or completion serial exhausted")]
    CounterExhausted,
    /// A completion cannot certify GPU use that was never successfully submitted.
    #[error("completion {completed} is beyond last submitted serial {submitted}")]
    FutureCompletion {
        /// Invalid reported serial.
        completed: u64,
        /// Latest successful submission.
        submitted: u64,
    },
    /// Device recreation is allowed only after an observed backend loss.
    #[error("device reset requires confirmed device loss")]
    DeviceNotLost,
    /// Reset must fence the exact next device generation in the same runtime.
    #[error("invalid replacement device generation")]
    InvalidDeviceGeneration,
    /// The caller still owns an old-device candidate and must discard it before recreation.
    #[error("discard the prepared generation before device recreation")]
    PreparedStillLive,
    /// Backend operation failed.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// Non-destructive installation rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InstallFailure {
    NotInitialized,
    StaleSource,
    RetirementFull,
    ForeignDevice,
    ForeignCore,
    StalePlan,
    DeviceLost,
}

pub(crate) struct InstallError<R> {
    reason: InstallFailure,
    prepared: PreparedGeneration<R>,
}

impl<R> InstallError<R> {
    pub(crate) fn reason(&self) -> InstallFailure {
        self.reason
    }
    pub(crate) fn into_prepared(self) -> PreparedGeneration<R> {
        self.prepared
    }
}

impl<R> fmt::Debug for InstallError<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstallError")
            .field("reason", &self.reason)
            .field("generation", &self.prepared.info)
            .finish()
    }
}

#[derive(Default)]
struct BudgetState {
    used: u64,
    peak: u64,
    prepared: usize,
}

struct BudgetLease {
    state: Rc<RefCell<BudgetState>>,
    bytes: u64,
    prepared: bool,
}

impl Drop for BudgetLease {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.used -= self.bytes;
        state.prepared -= usize::from(self.prepared);
    }
}

/// Kept only on the render worker; resources retire before their charge is released.
pub(crate) struct PreparedGeneration<R> {
    resources: Option<R>,
    plan: Option<CompiledPlan>,
    lease: Option<BudgetLease>,
    info: GenerationInfo,
}

impl<R> PreparedGeneration<R> {
    pub(crate) fn info(&self) -> GenerationInfo {
        self.info
    }
    pub(crate) fn plan(&self) -> &CompiledPlan {
        self.plan.as_ref().expect("live prepared plan")
    }

    fn installed(&mut self) {
        let lease = self.lease.as_mut().expect("live resource charge");
        lease.state.borrow_mut().prepared -= 1;
        lease.prepared = false;
    }

    fn into_plan(mut self) -> CompiledPlan {
        drop(self.resources.take());
        drop(self.lease.take());
        self.plan.take().expect("live prepared plan")
    }
}

impl<R> Drop for PreparedGeneration<R> {
    fn drop(&mut self) {
        drop(self.resources.take());
        drop(self.lease.take());
    }
}

struct LiveGeneration<R> {
    prepared: PreparedGeneration<R>,
    last_use: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceState {
    Uninitialized,
    Ready,
    Lost,
}

pub(crate) struct RenderCore<B: Backend> {
    backend: B,
    device: DeviceGeneration,
    limits: ResourceLimits,
    budget: Rc<RefCell<BudgetState>>,
    live: Option<LiveGeneration<B::Resources>>,
    retiring: Vec<LiveGeneration<B::Resources>>,
    in_flight: VecDeque<u64>,
    generation: u64,
    submitted: u64,
    completed: u64,
    device_state: DeviceState,
    rebuild: Option<(PlanIdentity, CompiledPlan)>,
    // Last field: even failed initialization/unwinding drops the backend before its charge.
    _base_lease: BudgetLease,
}

impl<B: Backend> RenderCore<B> {
    /// Reserves the complete baseline before native initialization, which may be retried.
    pub(crate) fn admit(backend: B, device: DeviceGeneration, limits: ResourceLimits) -> Result<Self, CoreError> {
        if limits.max_in_flight.get() > 2 {
            return Err(CoreError::InvalidLimits("at most two in-flight batches"));
        }
        if limits.max_retiring > 2 {
            return Err(CoreError::InvalidLimits("at most two retiring generations"));
        }
        let bytes = backend.base_resource_bytes()?;
        if bytes > limits.max_bytes {
            return Err(CoreError::ResourceBudgetExceeded {
                requested: bytes,
                used: 0,
                limit: limits.max_bytes,
            });
        }
        let budget = Rc::new(RefCell::new(BudgetState {
            used: bytes,
            peak: bytes,
            prepared: 0,
        }));
        let base_lease = BudgetLease {
            state: budget.clone(),
            bytes,
            prepared: false,
        };
        Ok(Self {
            backend,
            device,
            limits,
            budget,
            live: None,
            retiring: Vec::new(),
            in_flight: VecDeque::new(),
            generation: 0,
            submitted: 0,
            completed: 0,
            device_state: DeviceState::Uninitialized,
            rebuild: None,
            _base_lease: base_lease,
        })
    }

    pub(crate) fn initialize(&mut self) -> Result<(), CoreError> {
        match self.device_state {
            DeviceState::Ready => return Ok(()),
            DeviceState::Lost => return Err(CoreError::Backend(BackendError::DeviceLost)),
            DeviceState::Uninitialized => {}
        }
        // A failed first allocation is not a published device loss. Partial cleanup belongs
        // to the backend, while this original baseline charge and device identity stay held.
        self.backend.initialize()?;
        self.device_state = DeviceState::Ready;
        Ok(())
    }

    /// Distinguishes pending first allocation from later confirmed-loss/reset service.
    pub(crate) fn has_initialized(&self) -> bool {
        self.device_state != DeviceState::Uninitialized
    }

    pub(crate) fn prepare(
        &mut self,
        identity: PlanIdentity,
        plan: CompiledPlan,
    ) -> Result<PreparedGeneration<B::Resources>, CoreError> {
        if !plan.admission_is_current() {
            return Err(CoreError::StaleSource);
        }
        self.poll_completion()?;
        self.ensure_device()?;
        let generation = self.generation.checked_add(1).ok_or(CoreError::CounterExhausted)?;
        let bytes = self
            .backend
            .resource_bytes(&plan)
            .map_err(|error| self.backend_error(error))?;
        let lease = {
            let mut state = self.budget.borrow_mut();
            if state.prepared != 0 {
                return Err(CoreError::PreparationBusy);
            }
            let used = state
                .used
                .checked_add(bytes)
                .filter(|used| *used <= self.limits.max_bytes)
                .ok_or(CoreError::ResourceBudgetExceeded {
                    requested: bytes,
                    used: state.used,
                    limit: self.limits.max_bytes,
                })?;
            state.used = used;
            state.peak = state.peak.max(used);
            state.prepared = 1;
            BudgetLease {
                state: self.budget.clone(),
                bytes,
                prepared: true,
            }
        };
        let resources = self
            .backend
            .prepare(&plan, self.device)
            .map_err(|error| self.backend_error(error))?;
        self.generation = generation;
        let info = GenerationInfo {
            generation: GenerationId::new(generation),
            identity,
            device: self.device,
            bytes,
            output_count: plan.outputs().len(),
        };
        Ok(PreparedGeneration {
            resources: Some(resources),
            plan: Some(plan),
            lease: Some(lease),
            info,
        })
    }

    // Return the owned candidate inline: pressure must preserve it without allocating at a frame boundary.
    #[allow(clippy::result_large_err)]
    pub(crate) fn install(
        &mut self,
        mut prepared: PreparedGeneration<B::Resources>,
    ) -> Result<GenerationInfo, InstallError<B::Resources>> {
        let reason = if !prepared.plan().admission_is_current() {
            Some(InstallFailure::StaleSource)
        } else if self.device_state == DeviceState::Uninitialized {
            Some(InstallFailure::NotInitialized)
        } else if self.device_state == DeviceState::Lost {
            Some(InstallFailure::DeviceLost)
        } else if prepared.info.device != self.device {
            Some(InstallFailure::ForeignDevice)
        } else if !Rc::ptr_eq(&prepared.lease.as_ref().unwrap().state, &self.budget) {
            Some(InstallFailure::ForeignCore)
        } else if self.live.as_ref().is_some_and(|live| {
            live.prepared.info.identity.project == prepared.info.identity.project
                && live.prepared.info.identity.revision >= prepared.info.identity.revision
        }) {
            Some(InstallFailure::StalePlan)
        } else if self.live.as_ref().is_some_and(|live| live.last_use > self.completed)
            && self.retiring.len() >= self.limits.max_retiring
        {
            Some(InstallFailure::RetirementFull)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(InstallError { reason, prepared });
        }
        prepared.installed();
        let info = prepared.info;
        let old = self.live.replace(LiveGeneration { prepared, last_use: 0 });
        if let Some(old) = old {
            if old.last_use > self.completed {
                self.retiring.push(old);
            } else {
                drop(old);
            }
        }
        Ok(info)
    }

    pub(crate) fn submit(&mut self, time: TimeSnapshot) -> Result<Option<SubmissionInfo>, CoreError> {
        self.poll_completion()?;
        self.ensure_device()?;
        let Some(live) = self.live.as_ref() else {
            return Ok(None);
        };
        if live.prepared.info.output_count == 0 {
            return Ok(None);
        }
        if self.in_flight.len() >= self.limits.max_in_flight.get() {
            return Err(CoreError::InFlightFull);
        }
        let serial = self.submitted.checked_add(1).ok_or(CoreError::CounterExhausted)?;
        let token = CompletionToken {
            device: self.device,
            serial,
        };
        let info = live.prepared.info;
        let previous_use = live.last_use;
        // Claim before entering the backend: if it unwinds after queuing commands, Drop must
        // still drain this use. A pre-use rejection rolls the claim back below.
        self.submitted = serial;
        self.live.as_mut().unwrap().last_use = serial;
        self.in_flight.push_back(serial);
        let live = self.live.as_ref().unwrap();
        if let Err(error) = self.backend.submit(
            live.prepared.resources.as_ref().unwrap(),
            live.prepared.plan(),
            token,
            time,
        ) {
            self.in_flight.pop_back();
            self.submitted = serial - 1;
            self.live.as_mut().unwrap().last_use = previous_use;
            return Err(self.backend_error(error));
        }
        Ok(Some(SubmissionInfo {
            generation: info.generation,
            identity: info.identity,
            token,
        }))
    }

    pub(crate) fn poll_completion(&mut self) -> Result<(), CoreError> {
        self.ensure_device()?;
        let token = self.backend.completion().map_err(|error| self.backend_error(error))?;
        self.complete(token)?;
        Ok(())
    }

    pub(crate) fn complete(&mut self, token: CompletionToken) -> Result<bool, CoreError> {
        self.ensure_device()?;
        if token.device != self.device || token.serial < self.completed {
            return Ok(false);
        }
        if token.serial > self.submitted {
            return Err(CoreError::FutureCompletion {
                completed: token.serial,
                submitted: self.submitted,
            });
        }
        let changed = token.serial > self.completed;
        self.completed = token.serial;
        while self.in_flight.front().is_some_and(|serial| *serial <= self.completed) {
            self.in_flight.pop_front();
        }
        self.retiring.retain(|generation| generation.last_use > self.completed);
        Ok(changed)
    }

    pub(crate) fn clear_live(&mut self) -> Result<(), CoreError> {
        if self.live.as_ref().is_some_and(|live| live.last_use > self.completed)
            && self.retiring.len() >= self.limits.max_retiring
        {
            return Err(CoreError::RetirementFull);
        }
        if let Some(live) = self.live.take() {
            if live.last_use > self.completed {
                self.retiring.push(live);
            } else {
                drop(live);
            }
        }
        Ok(())
    }

    pub(crate) fn has_pending_uses(&self) -> bool {
        !self.in_flight.is_empty()
    }

    pub(crate) fn quiesce(&mut self) {
        self.backend.quiesce();
    }

    pub(crate) fn has_project(&self, project: super::identity::ProjectEpoch) -> bool {
        self.live
            .as_ref()
            .is_some_and(|live| live.prepared.info.identity.project == project)
            || self
                .retiring
                .iter()
                .any(|generation| generation.prepared.info.identity.project == project)
    }

    pub(crate) fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), CoreError> {
        self.ensure_device()?;
        self.backend
            .wait_for_completion(timeout)
            .map_err(|error| self.backend_error(error))
    }

    pub(crate) fn lose_device(&mut self) -> Result<(), CoreError> {
        if self.device_state != DeviceState::Lost {
            return Err(CoreError::DeviceNotLost);
        }
        if self.budget.borrow().prepared != 0 {
            return Err(CoreError::PreparedStillLive);
        }
        if let Some(live) = self.live.take() {
            self.rebuild = Some((live.prepared.info.identity, live.prepared.into_plan()));
        }
        self.retiring.clear();
        self.in_flight.clear();
        Ok(())
    }

    pub(crate) fn reset_after_loss(
        &mut self,
        device: DeviceGeneration,
    ) -> Result<Option<(PlanIdentity, CompiledPlan)>, CoreError> {
        if self.device.next() != Some(device) {
            return Err(CoreError::InvalidDeviceGeneration);
        }
        self.lose_device()?;
        self.backend.reset(device).map_err(|error| self.backend_error(error))?;
        self.device = device;
        self.submitted = 0;
        self.completed = 0;
        self.device_state = DeviceState::Ready;
        Ok(self.rebuild.take())
    }

    pub(crate) fn snapshot(&self) -> ResourceSnapshot {
        let budget = self.budget.borrow();
        ResourceSnapshot {
            byte_limit: self.limits.max_bytes,
            bytes_used: budget.used,
            peak_bytes: budget.peak,
            prepared_generations: budget.prepared,
            retiring_generations: self.retiring.len(),
            in_flight: self.in_flight.len(),
            active: self.live.as_ref().map(|live| live.prepared.info),
            last_submitted: CompletionToken {
                device: self.device,
                serial: self.submitted,
            },
            last_completed: CompletionToken {
                device: self.device,
                serial: self.completed,
            },
            initialized: self.has_initialized(),
            device_lost: self.device_state == DeviceState::Lost,
        }
    }

    fn ensure_device(&self) -> Result<(), CoreError> {
        match self.device_state {
            DeviceState::Uninitialized => Err(CoreError::NotInitialized),
            DeviceState::Ready => Ok(()),
            DeviceState::Lost => Err(CoreError::Backend(BackendError::DeviceLost)),
        }
    }

    fn backend_error(&mut self, error: BackendError) -> CoreError {
        if error == BackendError::DeviceLost {
            self.device_state = DeviceState::Lost;
        }
        CoreError::Backend(error)
    }
}

impl<B: Backend> Drop for RenderCore<B> {
    fn drop(&mut self) {
        self.quiesce();
        // This core is constructed and retained only by the render worker. Even an unwind
        // cannot certify retirement while native uses remain: keep its lifetime/window gate
        // held until completion or confirmed loss, then dispose resources before the backend.
        while self.has_pending_uses() && self.device_state != DeviceState::Lost {
            match self.poll_completion() {
                Ok(()) if self.has_pending_uses() => {
                    let _ = self.wait_for_completion(Duration::from_millis(10));
                }
                Ok(()) => {}
                Err(CoreError::Backend(BackendError::DeviceLost)) => break,
                Err(_) => {
                    let _ = self.wait_for_completion(Duration::from_millis(10));
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        self.in_flight.clear();
        drop(self.live.take());
        self.retiring.clear();
    }
}

#[cfg(test)]
mod tests;
