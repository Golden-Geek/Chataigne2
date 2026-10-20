use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
};

use golden_audio::{AudioCommand, AudioControl, AudioObservationReader};
use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision},
};
use golden_io::{BoundedWorker, PayloadPool, RetainedPayload, RetirementPermit, RetirementPool, WorkerLimits};

use super::super::{
    find_path, BuiltSoundCardConfiguration, ConfigurationError, RuntimeWakeSender, RuntimeWarning,
    SoundCardValueBindings,
};

const MAX_NODES: usize = 131_072;
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 64 * 1024 * 1024;
const SCRATCH_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    revisions: [Option<SubtreeRevision>; 3],
    uuid: golden_core::node::NodeUuid,
    enabled: bool,
}
impl Key {
    fn from(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Option<Self> {
        let node = snapshot.node(module)?;
        let revisions = ["connection", "parameters", "values"]
            .map(|path| find_path(snapshot, module, path).and_then(|root| snapshot.subtree_revision(root)));
        Some(Self {
            revisions,
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
    pub(crate) warnings: Vec<RuntimeWarning>,
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
    fn from_built(built: BuiltSoundCardConfiguration) -> Self {
        Self {
            configuration: Arc::new(built.configuration),
            sample_rate: built.sample_rate,
            bindings: built.bindings,
            warnings: built.warnings,
        }
    }
    fn weight(&self) -> Option<usize> {
        let mut bytes = self.configuration.retained_bytes_within(MAX_RESULT_BYTES, MAX_NODES)?
            + size_of::<Self>()
            + 2 * size_of::<usize>();
        bytes = bytes.checked_add(self.warnings.capacity() * size_of::<RuntimeWarning>())?;
        for warning in &self.warnings {
            bytes = bytes.checked_add(
                warning.id.capacity()
                    + warning.message.capacity()
                    + warning.detail.as_ref().map_or(0, String::capacity),
            )?;
            if bytes > MAX_RESULT_BYTES {
                return None;
            }
        }
        bytes = bytes.checked_add(
            self.bindings.channel_levels.capacity() * 128
                + self.bindings.pitch.capacity() * 256
                + self.bindings.runtime_values.capacity() * 64
                + 4096,
        )?;
        (bytes <= MAX_RESULT_BYTES).then_some(bytes)
    }
}

struct State {
    cancelled: AtomicBool,
    complete: AtomicBool,
    result: Mutex<Option<Result<RetainedPayload<PreparedConfiguration>, ConfigurationError>>>,
    wake: RuntimeWakeSender,
    _admission: RetirementPermit,
}
impl State {
    fn finish(&self, result: Result<RetainedPayload<PreparedConfiguration>, ConfigurationError>) {
        *self.result.lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
        self.complete.store(true, Ordering::Release);
        self.wake.wake();
    }
}

pub(crate) struct Preparation {
    key: Key,
    state: Arc<State>,
    ready: Option<RetainedPayload<PreparedConfiguration>>,
}
impl Preparation {
    pub(crate) fn matches(&self, snapshot: &ProcessTreeSnapshot, module: NodeId) -> bool {
        Some(self.key) == Key::from(snapshot, module)
    }
    pub(crate) fn start(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        reader: RetainedPayload<AudioObservationReader>,
        wake: RuntimeWakeSender,
    ) -> Result<Self, ConfigurationError> {
        let reject = |message| ConfigurationError {
            node: module,
            message,
            retryable: false,
        };
        let key = Key::from(snapshot, module)
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
            .submit(Job {
                snapshot,
                module,
                reader,
                state,
            })
            .map_err(|message| unavailable(module, message))?;
        Ok(request)
    }
    pub(crate) fn poll(&mut self) -> Result<Option<RetainedPayload<PreparedConfiguration>>, ConfigurationError> {
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
impl Drop for Preparation {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::Release);
    }
}

struct Job {
    snapshot: RetainedPayload<ProcessTreeSnapshot>,
    module: NodeId,
    reader: RetainedPayload<AudioObservationReader>,
    state: Arc<State>,
}
impl Job {
    fn run(&self, pool: &PayloadPool) -> Result<RetainedPayload<PreparedConfiguration>, ConfigurationError> {
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
            .reader
            .inspect(|snapshot| {
                snapshot.device.retained_bytes_within(8 * 1024 * 1024, MAX_NODES)?;
                Some(snapshot.device.clone())
            })
            .ok_or_else(|| reject("Sound Card device metadata exceeds 8 MiB / 131072 entries"))?;
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
        let data = PreparedConfiguration::from_built(built);
        let bytes = data
            .weight()
            .ok_or_else(|| reject("Sound Card prepared configuration exceeds 64 MiB"))?;
        if self.state.cancelled.load(Ordering::Acquire) {
            return Err(reject("Sound Card preparation superseded"));
        }
        reservation
            .retain(data, bytes)
            .map_err(|_| reject("Sound Card configuration reservation rejected"))
    }
}
impl Drop for Job {
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

struct Service {
    worker: Mutex<Option<BoundedWorker<Job, ()>>>,
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
    fn submit(&self, job: Job) -> Result<(), String> {
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
                BoundedWorker::<Job, ()>::spawn(
                    "sound-card-config-preparation",
                    &self.lifetimes,
                    WorkerLimits {
                        command_items: 16,
                        command_weight: 16 * size_of::<Job>(),
                        event_items: 1,
                        event_weight: 1,
                    },
                    move |context| {
                        while let Some(job) = context.receive() {
                            let mut result = job.run(&pool);
                            if let Err(error) = &mut result {
                                compact(&mut error.message);
                            }
                            job.state.finish(result);
                        }
                    },
                )
                .map_err(|error| error.to_string())?,
            );
        }
        worker
            .as_ref()
            .unwrap()
            .send_weighted(job, size_of::<Job>())
            .map_err(|_| "Sound Card configuration preparation admission is full or closed".into())
    }
}
static SERVICE: LazyLock<Service> = LazyLock::new(Service::new);

#[cfg(test)]
mod tests;
