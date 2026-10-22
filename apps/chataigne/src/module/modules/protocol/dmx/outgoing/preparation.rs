use super::*;
use golden_io::{BoundedWorker, RetirementPermit, RetirementPool, WorkerLimits, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

struct State {
    admitted: AtomicBool,
    cancelled: AtomicBool,
    complete: AtomicBool,
    result: Mutex<Option<Result<RetainedPayload<Prepared>, String>>>,
    wake: WorkerNotifier,
    _ticket: RetirementPermit,
}
impl State {
    fn finish(&self, result: Result<RetainedPayload<Prepared>, String>) {
        *self.result.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(result);
        self.complete.store(true, Ordering::Release);
        self.wake.notify();
    }
}
pub(super) struct Preparation(Arc<State>);
impl Preparation {
    pub(super) fn start(source: RetainedPayload<Source>, wake: WorkerNotifier) -> Result<Self, String> {
        static JOBS: OnceLock<RetirementPool> = OnceLock::new();
        let ticket = JOBS
            .get_or_init(|| RetirementPool::new(NonZeroUsize::new(32).unwrap()))
            .try_reserve()
            .map_err(|_| "DMX preparation capacity exhausted")?;
        let state = Arc::new(State {
            admitted: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            result: Mutex::new(None),
            wake,
            _ticket: ticket,
        });
        submit(Job {
            source,
            state: state.clone(),
        })?;
        Ok(Self(state))
    }
    pub(super) fn take(&self) -> Option<Result<RetainedPayload<Prepared>, String>> {
        self.0.result.lock().unwrap_or_else(|poison| poison.into_inner()).take()
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
}
struct Job {
    source: RetainedPayload<Source>,
    state: Arc<State>,
}
impl Job {
    fn run(self) {
        if self.state.cancelled.load(Ordering::Acquire) {
            return;
        }
        let result = (|| {
            let reservation = pool(true)?
                .try_reserve(WEIGHT)
                .map_err(|_| "DMX prepared payload capacity exhausted")?;
            let ready = decode::prepare(self.source.clone())?;
            reservation
                .retain(ready, WEIGHT)
                .map_err(|_| "DMX prepared payload reservation failed".into())
        })();
        if !self.state.cancelled.load(Ordering::Acquire) {
            self.state.finish(result);
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if self.state.admitted.load(Ordering::Acquire)
            && !self.state.cancelled.load(Ordering::Acquire)
            && !self.state.complete.load(Ordering::Acquire)
        {
            self.state
                .finish(Err("DMX preparation worker stopped; request was not sent".into()));
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
                "dmx-preparation",
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
    job.state.admitted.store(true, Ordering::Release);
    worker
        .as_ref()
        .unwrap()
        .send_weighted(job, size_of::<Job>())
        .map_err(|error| {
            let (golden_io::PendingSendError::Full(job) | golden_io::PendingSendError::Disconnected(job)) = error;
            job.state.admitted.store(false, Ordering::Release);
            "DMX preparation mailbox unavailable".into()
        })
}
