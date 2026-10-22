use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, LazyLock, Mutex,
    },
};

use golden_audio::{AudioCommand, AudioControl, AudioObservationReader};
use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision, SubtreeStructureRevision},
};
use golden_io::{BoundedWorker, PayloadPool, RetainedPayload, RetirementPermit, RetirementPool, WorkerLimits};

use super::super::observation::DeviceInventory;
use super::super::{
    find_path, BuiltSoundCardConfiguration, ConfigurationError, RuntimeWakeSender, SoundCardValueBindings,
};
use super::choices::ChoicePlan;
use super::selection::{ProbeTargets, SelectionPlan};
use super::warnings::WarningPlan;

const MAX_NODES: usize = 131_072;
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 64 * 1024 * 1024;
const SCRATCH_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    revisions: [Option<(NodeId, SubtreeRevision)>; 6],
    bindings: Option<SubtreeStructureRevision>,
    scope: Scope,
    uuid: golden_core::node::NodeUuid,
    enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Configuration,
    Selection,
    Choices,
}
impl Key {
    #[cfg(test)]
    fn from(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Option<Self> {
        Self::read(snapshot, module, Scope::Configuration)
    }
    fn read(snapshot: &ProcessTreeSnapshot, module: NodeId, scope: Scope) -> Option<Self> {
        let node = snapshot.node(module)?;
        let paths = match scope {
            Scope::Configuration => [Some("connection"), Some("parameters"), None, None, None, None],
            Scope::Choices => [
                Some("connection/audio_driver"),
                Some("connection/device"),
                Some("connection/input_device"),
                Some("connection/output_device"),
                Some("connection/sample_rate"),
                Some("connection/buffer_size"),
            ],
            Scope::Selection => [
                Some("connection/audio_driver"),
                Some("connection/device"),
                Some("connection/input_device"),
                Some("connection/output_device"),
                None,
                None,
            ],
        };
        let revisions = paths.map(|path| {
            path.and_then(|path| find_path(snapshot, module, path))
                .and_then(|root| snapshot.subtree_revision(root).map(|revision| (root, revision)))
        });
        Some(Self {
            revisions,
            bindings: if scope == Scope::Configuration {
                find_path(snapshot, module, "values").and_then(|root| snapshot.subtree_structure_revision(root))
            } else {
                None
            },
            scope,
            uuid: node.uuid,
            enabled: node.enabled,
        })
    }
}

#[derive(Debug)]
pub(crate) struct PreparedConfiguration {
    configuration: Arc<golden_audio::AudioConfiguration>,
    pub(crate) sample_rate: golden_audio::SampleRate,
    pub(crate) bindings: SoundCardValueBindings,
    pub(crate) warnings: RetainedPayload<WarningPlan>,
    pub(crate) value_cache: Mutex<super::super::observation::ValueCache>,
}
impl PreparedConfiguration {
    /// The preparing lease remains borrowed during rejection, so dropping the
    /// command's Arc cannot dispose the full configuration on this caller.
    pub(crate) fn submit(
        &self,
        control: &AudioControl,
        generation: golden_audio::ConfigGeneration,
    ) -> Result<golden_audio::CommandSequence, golden_audio::AudioError> {
        control.submit(AudioCommand::ApplyConfiguration {
            generation,
            config: self.configuration.clone(),
        })
    }
    fn from_built(
        built: BuiltSoundCardConfiguration,
        warnings: RetainedPayload<WarningPlan>,
    ) -> Result<Self, &'static str> {
        let value_cache = Mutex::new(super::super::observation::ValueCache::prepare(&built.bindings)?);
        Ok(Self {
            configuration: Arc::new(built.configuration),
            sample_rate: built.sample_rate,
            bindings: built.bindings,
            warnings,
            value_cache,
        })
    }
    fn weight(&self) -> Option<usize> {
        let mut bytes = self.configuration.retained_bytes_within(MAX_RESULT_BYTES, MAX_NODES)?
            + size_of::<Self>()
            + 2 * size_of::<usize>();
        bytes = bytes.checked_add(
            self.bindings.channel_levels.capacity() * 128
                + self.bindings.pitch.capacity() * 256
                + self.bindings.runtime_values.capacity() * 64
                + 4096,
        )?;
        bytes = bytes.checked_add(
            self.value_cache
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .retained_heap_bytes(),
        )?;
        (bytes <= MAX_RESULT_BYTES).then_some(bytes)
    }
}

struct State<T: Send + Sync + 'static> {
    device_captured: AtomicBool,
    device_revision: AtomicU64,
    cancelled: AtomicBool,
    complete: AtomicBool,
    result: Mutex<Option<Result<RetainedPayload<T>, ConfigurationError>>>,
    wake: RuntimeWakeSender,
    _admission: RetirementPermit,
}
impl<T: Send + Sync + 'static> State<T> {
    fn finish(&self, result: Result<RetainedPayload<T>, ConfigurationError>) {
        *self.result.lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
        self.complete.store(true, Ordering::Release);
        self.wake.wake();
    }
}

pub(crate) struct Preparation<T: Send + Sync + 'static = PreparedConfiguration> {
    key: Key,
    state: Arc<State<T>>,
    ready: Option<RetainedPayload<T>>,
}
struct Mode<T: Send + Sync + 'static, S> {
    scope: Scope,
    wrap: fn(Job<T, S>) -> Work,
}
impl Preparation<PreparedConfiguration> {
    pub(crate) fn start(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        reader: RetainedPayload<AudioObservationReader>,
        wake: RuntimeWakeSender,
    ) -> Result<Self, ConfigurationError> {
        Self::start_with(
            snapshot,
            module,
            reader,
            wake,
            None,
            Mode {
                scope: Scope::Configuration,
                wrap: Work::Configuration,
            },
        )
    }
}
impl Preparation<SelectionPlan> {
    pub(crate) fn start_selection(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        reader: RetainedPayload<AudioObservationReader>,
        wake: RuntimeWakeSender,
        previous: Option<ProbeTargets>,
    ) -> Result<Self, ConfigurationError> {
        Self::start_with(
            snapshot,
            module,
            reader,
            wake,
            previous,
            Mode {
                scope: Scope::Selection,
                wrap: Work::Selection,
            },
        )
    }
}
impl Preparation<ChoicePlan> {
    pub(crate) fn start_choices(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        inventory: DeviceInventory,
        wake: RuntimeWakeSender,
    ) -> Result<Self, ConfigurationError> {
        let revision = inventory.revision();
        let request = Self::start_with(
            snapshot,
            module,
            inventory,
            wake,
            None,
            Mode {
                scope: Scope::Choices,
                wrap: Work::Choices,
            },
        )?;
        // This source is already immutable, including while its job is queued.
        request.state.device_revision.store(revision, Ordering::Release);
        request.state.device_captured.store(true, Ordering::Release);
        Ok(request)
    }
}
impl<T: Send + Sync + 'static> Preparation<T> {
    pub(crate) fn is_complete(&self) -> bool {
        self.state.complete.load(Ordering::Acquire)
    }

    pub(crate) fn matches_device_revision(&self, revision: u64) -> bool {
        !self.state.device_captured.load(Ordering::Acquire)
            || self.state.device_revision.load(Ordering::Acquire) == revision
    }
    pub(crate) fn matches(&self, snapshot: &ProcessTreeSnapshot, module: NodeId) -> bool {
        Some(self.key) == Key::read(snapshot, module, self.key.scope)
    }
    fn start_with<S>(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        source: S,
        wake: RuntimeWakeSender,
        previous: Option<ProbeTargets>,
        mode: Mode<T, S>,
    ) -> Result<Self, ConfigurationError> {
        let reject = |message| ConfigurationError {
            node: module,
            message,
            retryable: false,
        };
        let key = Key::read(snapshot, module, mode.scope)
            .filter(|key| key.enabled)
            .ok_or_else(|| reject("Sound Card preparation source is absent or disabled".into()))?;
        let admission = SERVICE
            .admission
            .try_reserve()
            .map_err(|error| unavailable(module, error.to_string()))?;
        let snapshot = snapshot
            .try_lease()
            .map_err(|error| unavailable(module, error.to_string()))?;
        let state = Arc::new(State {
            device_captured: AtomicBool::new(false),
            device_revision: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            result: Mutex::new(None),
            wake,
            _admission: admission,
        });
        let request = Self {
            key,
            state: state.clone(),
            ready: None,
        };
        SERVICE
            .submit((mode.wrap)(Job {
                previous,
                snapshot,
                module,
                source,
                state,
            }))
            .map_err(|message| unavailable(module, message))?;
        Ok(request)
    }
    pub(crate) fn poll(&mut self) -> Result<Option<RetainedPayload<T>>, ConfigurationError> {
        if let Some(ready) = &self.ready {
            return Ok(Some(ready.clone()));
        }
        let result = self.state.result.lock().unwrap_or_else(|p| p.into_inner()).take();
        match result {
            Some(Ok(ready)) => {
                self.ready = Some(ready.clone());
                Ok(Some(ready))
            }
            Some(Err(error)) => Err(error),
            None => Ok(None),
        }
    }
}
impl<T: Send + Sync + 'static> Drop for Preparation<T> {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::Release);
    }
}

struct Job<T: Send + Sync + 'static = PreparedConfiguration, S = RetainedPayload<AudioObservationReader>> {
    previous: Option<ProbeTargets>,
    snapshot: RetainedPayload<ProcessTreeSnapshot>,
    module: NodeId,
    source: S,
    state: Arc<State<T>>,
}
impl<T: Send + Sync + 'static> Job<T> {
    fn capture(
        &self,
        pool: &PayloadPool,
    ) -> Result<
        (
            golden_io::PayloadReservation,
            golden_audio::AudioDeviceInspectorState,
            u64,
        ),
        ConfigurationError,
    > {
        let reject = |message: &str| ConfigurationError {
            node: self.module,
            retryable: false,
            message: message.into(),
        };
        if self.state.cancelled.load(Ordering::Acquire) {
            return Err(reject("Sound Card preparation cancelled"));
        }
        validate_source(&self.snapshot, self.module)?;
        let reservation = pool.try_reserve(SCRATCH_BYTES).map_err(|_| {
            unavailable(
                self.module,
                "Sound Card configuration lifetime capacity exhausted".into(),
            )
        })?;
        let inspector = self
            .source
            .inspect(|snapshot| {
                self.state
                    .device_revision
                    .store(snapshot.device_revision, Ordering::Release);
                self.state.device_captured.store(true, Ordering::Release);
                snapshot.device.retained_bytes_within(8 * 1024 * 1024, MAX_NODES)?;
                Some((snapshot.device.clone(), snapshot.device_revision))
            })
            .ok_or_else(|| {
                unavailable(
                    self.module,
                    "Sound Card device metadata exceeds 8 MiB / 131072 entries".into(),
                )
            })?;
        Ok((reservation, inspector.0, inspector.1))
    }
}
impl<T: Send + Sync + 'static, S> Job<T, S> {
    fn finish(&self, mut result: Result<RetainedPayload<T>, ConfigurationError>) {
        if let Err(error) = &mut result {
            compact(&mut error.message);
        }
        self.state.finish(result);
    }
}
impl Job<PreparedConfiguration> {
    fn run(&self, pool: &PayloadPool) -> Result<RetainedPayload<PreparedConfiguration>, ConfigurationError> {
        let reject = |message: &str| ConfigurationError {
            node: self.module,
            retryable: false,
            message: message.into(),
        };
        let (reservation, inspector, _revision) = self.capture(pool)?;
        let mut built = super::build_configuration(&self.snapshot, self.module, &inspector).map_err(|mut error| {
            compact(&mut error.message);
            error
        })?;
        for warning in &mut built.warnings {
            compact(&mut warning.message);
            if let Some(detail) = &mut warning.detail {
                compact(detail);
            }
        }
        drop(inspector);
        let warnings = WarningPlan::prepare(&self.snapshot, self.module, &built.warnings)
            .filter(|plan| plan.bytes <= MAX_RESULT_BYTES)
            .ok_or_else(|| {
                unavailable(
                    self.module,
                    "Sound Card warning reconciliation exceeds its item/storage bound".into(),
                )
            })?;
        built.warnings.clear();
        let warning_bytes = warnings.bytes;
        let warnings = pool.try_retain(warnings, warning_bytes).map_err(|_| {
            unavailable(
                self.module,
                "Sound Card warning-plan lifetime capacity exhausted".into(),
            )
        })?;
        let data = PreparedConfiguration::from_built(built, warnings).map_err(reject)?;
        let bytes = data
            .weight()
            .ok_or_else(|| reject("Sound Card prepared configuration exceeds 64 MiB"))?;
        if bytes
            .checked_add(data.warnings.bytes)
            .is_none_or(|bytes| bytes > MAX_RESULT_BYTES)
        {
            return Err(unavailable(
                self.module,
                "Sound Card configuration and warning plan exceed 64 MiB".into(),
            ));
        }
        if self.state.cancelled.load(Ordering::Acquire) {
            return Err(reject("Sound Card preparation superseded"));
        }
        reservation
            .retain(data, bytes)
            .map_err(|_| reject("Sound Card configuration reservation rejected"))
    }
}
impl Job<SelectionPlan> {
    fn run(&self, pool: &PayloadPool) -> Result<RetainedPayload<SelectionPlan>, ConfigurationError> {
        let (reservation, inspector, revision) = self.capture(pool)?;
        let data = SelectionPlan::prepare(
            &self.snapshot,
            self.module,
            &inspector,
            revision,
            self.previous.as_ref(),
            pool,
        )
        .map_err(|mut error| {
            compact(&mut error.message);
            error
        })?;
        drop(inspector);
        if self.state.cancelled.load(Ordering::Acquire) {
            return Err(unavailable(
                self.module,
                "Sound Card selection preparation superseded".into(),
            ));
        }
        let bytes = data.bytes;
        reservation
            .retain(data, bytes)
            .map_err(|_| unavailable(self.module, "Sound Card selection reservation rejected".into()))
    }
}
impl Job<ChoicePlan, DeviceInventory> {
    fn run(&self, pool: &PayloadPool) -> Result<RetainedPayload<ChoicePlan>, ConfigurationError> {
        let revision = self.source.revision();
        self.state.device_revision.store(revision, Ordering::Release);
        self.state.device_captured.store(true, Ordering::Release);
        if self.state.cancelled.load(Ordering::Acquire) {
            return Err(unavailable(self.module, "Sound Card choices superseded".into()));
        }
        let reservation = pool
            .try_reserve(SCRATCH_BYTES)
            .map_err(|_| unavailable(self.module, "Sound Card choices lifetime capacity exhausted".into()))?;
        let options = super::choices::option_pool()
            .map_err(|error| unavailable(self.module, format!("Sound Card enum storage unavailable: {error}")))?;
        let plan = ChoicePlan::prepare(&self.snapshot, self.module, &self.source, &options)?;
        let bytes = plan.bytes;
        if self.state.cancelled.load(Ordering::Acquire) {
            return Err(unavailable(self.module, "Sound Card choices superseded".into()));
        }
        reservation
            .retain(plan, bytes)
            .map_err(|_| unavailable(self.module, "Sound Card choices reservation rejected".into()))
    }
}
impl<T: Send + Sync + 'static, S> Drop for Job<T, S> {
    fn drop(&mut self) {
        if !self.state.complete.load(Ordering::Acquire) && !self.state.cancelled.load(Ordering::Acquire) {
            self.state.finish(Err(ConfigurationError {
                node: self.module,
                retryable: true,
                message: "Sound Card preparation worker stopped before completion".into(),
            }));
        }
    }
}

fn compact(text: &mut String) {
    if text.len() > 4096 {
        let mut end = 4080;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(" [truncated]");
    }
    text.shrink_to_fit();
}

fn validate_source(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Result<(), ConfigurationError> {
    let reject = || ConfigurationError {
        node: module,
        retryable: false,
        message: "Sound Card configuration source exceeds 131072 nodes / 32 MiB of labels and values".into(),
    };
    let mut pending = vec![module];
    let mut visited = 0usize;
    let mut bytes = 0usize;
    while let Some(id) = pending.pop() {
        visited += 1;
        let node = snapshot.node(id).ok_or_else(reject)?;
        let value = match &node.param_value {
            Some(ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value)) => value.len(),
            Some(ParamValue::Reference(value)) => value.cached_name().map_or(0, str::len),
            _ => 0,
        };
        bytes = bytes.saturating_add(node.label.len()).saturating_add(value);
        let children = snapshot.child_ids_slice(id);
        if visited + pending.len() + children.len() > MAX_NODES
            || bytes > MAX_INPUT_BYTES
            || value > 1024 * 1024
            || node.label.len() > 1024 * 1024
        {
            return Err(reject());
        }
        pending.extend_from_slice(children);
    }
    Ok(())
}

fn unavailable(node: NodeId, message: String) -> ConfigurationError {
    ConfigurationError {
        node,
        message,
        retryable: true,
    }
}

enum Work {
    Configuration(Job),
    Selection(Job<SelectionPlan>),
    Choices(Job<ChoicePlan, DeviceInventory>),
}
impl Work {
    fn run(self, pool: &PayloadPool) {
        match self {
            Self::Configuration(job) => job.finish(job.run(pool)),
            Self::Selection(job) => job.finish(job.run(pool)),
            Self::Choices(job) => job.finish(job.run(pool)),
        }
    }
}
struct Service {
    worker: Mutex<Option<BoundedWorker<Work, ()>>>,
    lifetimes: RetirementPool,
    admission: RetirementPool,
    pool: Mutex<Option<PayloadPool>>,
}
impl Service {
    fn new() -> Self {
        Self {
            worker: Mutex::new(None),
            lifetimes: RetirementPool::new(NonZeroUsize::MIN),
            admission: RetirementPool::new(NonZeroUsize::new(32).unwrap()),
            pool: Mutex::new(None),
        }
    }
    fn submit(&self, job: Work) -> Result<(), String> {
        let mut worker = self.worker.lock().unwrap_or_else(|p| p.into_inner());
        if worker.as_ref().is_some_and(|worker| worker.has_pending()) {
            *worker = None;
        }
        if worker.is_none() {
            let mut pool = self.pool.lock().unwrap_or_else(|p| p.into_inner());
            if pool.is_none() {
                *pool = Some(
                    PayloadPool::new("sound-card-config-disposer", 128, 1024 * 1024 * 1024)
                        .map_err(|error| error.to_string())?,
                );
            }
            let pool = pool.as_ref().unwrap().clone();
            *worker = Some(
                BoundedWorker::<Work, ()>::spawn(
                    "sound-card-config-preparation",
                    &self.lifetimes,
                    WorkerLimits {
                        command_items: 16,
                        command_weight: 16 * size_of::<Work>(),
                        event_items: 1,
                        event_weight: 1,
                    },
                    move |context| {
                        while let Some(job) = context.receive() {
                            job.run(&pool);
                        }
                    },
                )
                .map_err(|error| error.to_string())?,
            );
        }
        worker
            .as_ref()
            .unwrap()
            .send_weighted(job, size_of::<Work>())
            .map_err(|_| "Sound Card configuration preparation admission is full or closed".into())
    }
}
static SERVICE: LazyLock<Service> = LazyLock::new(Service::new);

#[cfg(test)]
mod tests;
