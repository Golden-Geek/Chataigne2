use super::*;
use golden_io::{BoundedWorker, RetirementPermit, RetirementPool, WorkerLimits, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

struct State {
    cancelled: AtomicBool,
    complete: AtomicBool,
    result: Mutex<Option<Result<Ready, Error>>>,
    wake: WorkerNotifier,
    _admission: RetirementPermit,
}
impl State {
    fn finish(&self, result: Result<Ready, Error>) {
        *self.result.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(result);
        self.complete.store(true, Ordering::Release);
        self.wake.notify();
    }
}

pub(in super::super) struct Preparation {
    key: Key,
    state: Arc<State>,
    ready: Option<Ready>,
}
impl Preparation {
    pub(super) fn start(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        previous: Option<RetainedPayload<MqttTransportConfig>>,
        wake: WorkerNotifier,
    ) -> Result<Self, Error> {
        static JOBS: OnceLock<RetirementPool> = OnceLock::new();
        let admission = JOBS.get_or_init(|| RetirementPool::new(NonZeroUsize::new(32).unwrap()));
        let (request, job) = Self::draft(snapshot, module, previous, wake, admission)?;
        submit(job)?;
        Ok(request)
    }
    pub(super) fn draft(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        previous: Option<RetainedPayload<MqttTransportConfig>>,
        wake: WorkerNotifier,
        admission: &RetirementPool,
    ) -> Result<(Self, Job), Error> {
        let key = Key::capture(snapshot, module).ok_or_else(|| Error::invalid("MQTT module disappeared"))?;
        let permit = admission
            .try_reserve()
            .map_err(|_| Error::unavailable("MQTT configuration preparation capacity exhausted"))?;
        let snapshot = snapshot
            .try_lease()
            .map_err(|error| Error::unavailable(error.to_string()))?;
        let state = Arc::new(State {
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            result: Mutex::new(None),
            wake,
            _admission: permit,
        });
        Ok((
            Self {
                key,
                state: state.clone(),
                ready: None,
            },
            Job {
                snapshot,
                module,
                previous,
                state,
            },
        ))
    }
    pub(super) fn is_complete(&self) -> bool {
        self.state.complete.load(Ordering::Acquire)
    }
    pub(super) fn matches(&self, snapshot: &ProcessTreeSnapshot, module: NodeId) -> bool {
        Key::capture(snapshot, module) == Some(self.key)
    }
    pub(super) fn poll(&mut self) -> Result<Option<(RetainedPayload<MqttTransportConfig>, bool)>, Error> {
        if self.ready.is_none() {
            let result = self
                .state
                .result
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take();
            if let Some(result) = result {
                self.ready = Some(result?);
            }
        }
        Ok(self.ready.as_ref().map(|ready| (ready.config.clone(), ready.unchanged)))
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::Release);
    }
}

pub(super) struct Job {
    snapshot: RetainedPayload<ProcessTreeSnapshot>,
    module: NodeId,
    previous: Option<RetainedPayload<MqttTransportConfig>>,
    state: Arc<State>,
}
impl Job {
    pub(super) fn run(self) {
        if self.state.cancelled.load(Ordering::Acquire) {
            return;
        }
        let result = (|| {
            let config = collect::build(&self.snapshot, self.module, &self.state.cancelled)?;
            let unchanged = self.previous.as_ref().is_some_and(|previous| **previous == config);
            let config = if unchanged {
                self.previous.as_ref().unwrap().clone()
            } else {
                super::super::transport::retain_configuration(config).map_err(|error| match error {
                    super::super::transport::ConfigAdmissionError::Invalid(message) => Error::invalid(message),
                    super::super::transport::ConfigAdmissionError::Unavailable(message) => Error::unavailable(message),
                })?
            };
            Ok(Ready { config, unchanged })
        })();
        if !self.state.cancelled.load(Ordering::Acquire) {
            self.state.finish(result);
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if !self.state.cancelled.load(Ordering::Acquire) && !self.state.complete.load(Ordering::Acquire) {
            self.state.finish(Err(Error::unavailable(
                "MQTT configuration worker stopped before completion",
            )));
        }
    }
}

fn submit(job: Job) -> Result<(), Error> {
    static WORKER: OnceLock<Mutex<Option<BoundedWorker<Job, ()>>>> = OnceLock::new();
    static LIFETIME: OnceLock<RetirementPool> = OnceLock::new();
    let mut worker = WORKER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if worker.as_ref().is_some_and(|worker| !worker.is_accepting_commands()) {
        *worker = None;
    }
    if worker.is_none() {
        let lifetime = LIFETIME.get_or_init(|| RetirementPool::new(NonZeroUsize::MIN));
        *worker = Some(
            BoundedWorker::<Job, ()>::spawn(
                "mqtt-configuration",
                lifetime,
                WorkerLimits {
                    command_items: 32,
                    command_weight: 32 * size_of::<Job>(),
                    event_items: 1,
                    event_weight: 1,
                },
                |ctx| {
                    while let Some(job) = ctx.receive() {
                        job.run();
                    }
                },
            )
            .map_err(|error| Error::unavailable(error.to_string()))?,
        );
    }
    worker
        .as_ref()
        .unwrap()
        .send_weighted(job, size_of::<Job>())
        .map_err(|_| Error::unavailable("MQTT configuration worker mailbox unavailable"))
}
