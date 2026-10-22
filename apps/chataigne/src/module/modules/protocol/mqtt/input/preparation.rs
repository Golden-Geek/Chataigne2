use super::*;
use crate::app::module::script_api::NativeCallbackPreparer;
use golden_io::{BoundedWorker, PayloadPool, RetirementPermit, RetirementPool, WorkerLimits};
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
    pressured: AtomicBool,
    result: Mutex<Option<Result<RetainedPayload<Prepared>, String>>>,
    wake: WorkerNotifier,
    _permit: RetirementPermit,
}
impl State {
    fn finish(&self, result: Result<RetainedPayload<Prepared>, String>) {
        *self.result.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(result);
        self.complete.store(true, Ordering::Release);
        self.wake.notify();
    }
}
pub(super) struct Preparation {
    state: Arc<State>,
}
impl Preparation {
    pub(super) fn start(
        raw: RetainedPayload<MqttReceivedPublish>,
        source: Source,
        log: bool,
        wake: WorkerNotifier,
    ) -> Result<Self, String> {
        static JOBS: OnceLock<RetirementPool> = OnceLock::new();
        let pool = JOBS.get_or_init(|| RetirementPool::new(NonZeroUsize::new(32).unwrap()));
        let (request, job) = Self::draft(raw, source, log, wake, pool)?;
        submit(job)?;
        Ok(request)
    }
    pub(super) fn draft(
        raw: RetainedPayload<MqttReceivedPublish>,
        source: Source,
        log: bool,
        wake: WorkerNotifier,
        pool: &RetirementPool,
    ) -> Result<(Self, Job), String> {
        let state = Arc::new(State {
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            pressured: AtomicBool::new(false),
            result: Mutex::new(None),
            wake,
            _permit: pool
                .try_reserve()
                .map_err(|_| "MQTT input preparation capacity exhausted")?,
        });
        Ok((
            Self { state: state.clone() },
            Job {
                raw,
                source: Some(source),
                log,
                state,
            },
        ))
    }
    pub(super) fn is_complete(&self) -> bool {
        self.state.complete.load(Ordering::Acquire)
    }
    pub(super) fn is_pressured(&self) -> bool {
        self.state.pressured.swap(false, Ordering::AcqRel)
    }
    pub(super) fn poll(&mut self) -> Option<Result<RetainedPayload<Prepared>, String>> {
        self.state
            .result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::Release);
    }
}

pub(super) struct Job {
    raw: RetainedPayload<MqttReceivedPublish>,
    source: Option<Source>,
    log: bool,
    state: Arc<State>,
}
impl Job {
    pub(super) fn run(mut self) {
        if self.state.cancelled.load(Ordering::Acquire) {
            return;
        }
        let result = (|| {
            let values = value_pool()?;
            let callbacks = NativeCallbackPreparer::new()?;
            let mut draft = build::Draft::new(self.raw.clone(), self.source.take().unwrap(), self.log)?;
            let mut notified = false;
            while !self.state.cancelled.load(Ordering::Acquire) {
                if let Some(input) = draft.retain(&values, &callbacks) {
                    return Ok(input);
                }
                if !notified {
                    self.state.pressured.store(true, Ordering::Release);
                    self.state.wake.notify();
                    notified = true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err("MQTT input preparation cancelled".into())
        })();
        if !self.state.cancelled.load(Ordering::Acquire) {
            self.state.finish(result);
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if !self.state.complete.load(Ordering::Acquire) && !self.state.cancelled.load(Ordering::Acquire) {
            self.state.finish(Err(
                "MQTT input preparation worker stopped; message was not published".into()
            ));
        }
    }
}
fn submit(job: Job) -> Result<(), String> {
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
        *worker = Some(
            BoundedWorker::<Job, ()>::spawn(
                "mqtt-input",
                LIFETIME.get_or_init(|| RetirementPool::new(NonZeroUsize::MIN)),
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
            .map_err(|error| error.to_string())?,
        );
    }
    worker
        .as_ref()
        .unwrap()
        .send_weighted(job, size_of::<Job>())
        .map_err(|_| "MQTT input worker mailbox is unavailable".into())
}
fn value_pool() -> Result<PayloadPool, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|poison| poison.into_inner());
    if pool.is_none() {
        *pool =
            Some(PayloadPool::new("mqtt-prepared-input", 512, 64 * 1024 * 1024).map_err(|error| error.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
