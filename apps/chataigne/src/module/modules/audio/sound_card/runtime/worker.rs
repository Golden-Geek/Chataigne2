use std::{
    fmt,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};

use golden_audio::{BackendId, SampleRate};
use golden_io::{BoundedWorker, PendingDrainState, PendingSendError, RetirementPool, WorkerLimits, WorkerNotifier};

use super::{RuntimeWakeSender, SoundCardRuntime};

const START_COALESCE_WINDOW: Duration = Duration::from_millis(10);
const LIFECYCLE_COMMAND_CAPACITY: usize = 8;
const MAX_DRIVER_BYTES: usize = 128;
const MAX_START_ERROR_BYTES: usize = 4096;

fn lifecycle_worker_lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(8).unwrap()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SoundCardRuntimeRequest {
    id: u64,
    sample_rate: SampleRate,
    driver: Option<BackendId>,
}
impl SoundCardRuntimeRequest {
    pub(crate) const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }
    pub(crate) fn driver(&self) -> Option<&BackendId> {
        self.driver.as_ref()
    }
}

#[derive(Debug)]
pub(crate) struct SoundCardRuntimeStarted {
    pub request: SoundCardRuntimeRequest,
    pub result: Result<Box<SoundCardRuntime>, String>,
}
pub(crate) enum SoundCardRuntimeWorkerPoll {
    Pending,
    Started(SoundCardRuntimeStarted),
    Disconnected,
}

#[derive(Debug)]
pub(crate) enum StartRequestError {
    Full,
    Disconnected,
    Invalid(&'static str),
}
impl fmt::Display for StartRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Full => "Sound Card runtime worker queue is full",
            Self::Disconnected => "Sound Card runtime worker stopped",
            Self::Invalid(message) => message,
        })
    }
}

pub(crate) struct SoundCardRuntimeWorker {
    worker: BoundedWorker<SoundCardRuntimeRequest, SoundCardRuntimeStarted>,
    next_request_id: u64,
    enabled: Arc<AtomicBool>,
}

impl SoundCardRuntimeWorker {
    pub(crate) fn spawn(wake: RuntimeWakeSender) -> Result<Self, String> {
        Self::spawn_using(wake, lifecycle_worker_lifetimes(), SoundCardRuntime::start_selected)
    }

    pub(in super::super) fn spawn_using<F>(
        wake: RuntimeWakeSender,
        lifetimes: &RetirementPool,
        starter: F,
    ) -> Result<Self, String>
    where
        F: Fn(SampleRate, Option<BackendId>) -> Result<SoundCardRuntime, String> + Send + 'static,
    {
        let command_weight = std::mem::size_of::<SoundCardRuntimeRequest>() + MAX_DRIVER_BYTES;
        let event_weight = std::mem::size_of::<SoundCardRuntimeStarted>()
            + std::mem::size_of::<SoundCardRuntime>()
            + MAX_START_ERROR_BYTES;
        let enabled = Arc::new(AtomicBool::new(true));
        let worker_enabled = enabled.clone();
        let worker = BoundedWorker::<SoundCardRuntimeRequest, SoundCardRuntimeStarted>::spawn_notified(
            "chataigne-sound-card-runtime",
            lifetimes,
            WorkerLimits {
                command_items: LIFECYCLE_COMMAND_CAPACITY,
                command_weight: LIFECYCLE_COMMAND_CAPACITY * command_weight,
                event_items: 2,
                event_weight: 2 * event_weight,
            },
            WorkerNotifier::new(move || wake.wake()),
            move |context| {
                let mut previous: Option<RetirementPool> = None;
                while let Some(mut request) = context.receive() {
                    // One fixed window plus one bounded drain, so continuous edits
                    // cannot postpone startup forever by restarting the timeout.
                    if context.cancellation().wait(START_COALESCE_WINDOW) {
                        return;
                    }
                    if let Some(previous) = &previous {
                        while !previous.wait_for_idle(Duration::from_millis(100)) {
                            if context.cancellation().is_cancelled() {
                                return;
                            }
                        }
                    }
                    let mut newer = Vec::with_capacity(LIFECYCLE_COMMAND_CAPACITY);
                    context.drain_commands(&mut newer, NonZeroUsize::new(LIFECYCLE_COMMAND_CAPACITY).unwrap());
                    if let Some(latest) = newer.pop() {
                        request = latest;
                    }
                    if context.cancellation().is_cancelled() {
                        return;
                    }
                    // An idle selection retires this worker only after the previous
                    // native lifetime completed. Newer selections can supersede it
                    // in the bounded coalescing drain above.
                    if request.driver.is_none() {
                        return;
                    }
                    if !worker_enabled.load(Ordering::Acquire) {
                        continue;
                    }
                    let result = starter(request.sample_rate, request.driver.clone())
                        .map(Box::new)
                        .map_err(|error| bounded_error(&error));
                    previous = result.as_ref().ok().map(|runtime| runtime.completion());
                    if !worker_enabled.load(Ordering::Acquire) {
                        drop(result);
                        continue;
                    }
                    let started = SoundCardRuntimeStarted { request, result };
                    if context.publish_weighted(started, event_weight).is_err() {
                        return;
                    }
                }
            },
        )
        .map_err(|error| format!("failed to start Sound Card lifecycle worker: {error}"))?;
        Ok(Self {
            worker,
            next_request_id: 1,
            enabled,
        })
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    pub(crate) fn request_start_for_driver(
        &mut self,
        sample_rate: SampleRate,
        driver: Option<BackendId>,
    ) -> Result<SoundCardRuntimeRequest, StartRequestError> {
        if driver
            .as_ref()
            .is_some_and(|driver| driver.as_str().len() > MAX_DRIVER_BYTES)
        {
            return Err(StartRequestError::Invalid(
                "Sound Card driver identifier exceeds 128 bytes",
            ));
        }
        // Idle is an idempotent desired state, not a new startup identity. It
        // remains admissible even after startup identities have been exhausted.
        let next = if driver.is_some() {
            self.next_request_id.checked_add(1).ok_or(StartRequestError::Invalid(
                "Sound Card runtime request identity exhausted",
            ))?
        } else {
            self.next_request_id
        };
        let request = SoundCardRuntimeRequest {
            id: if driver.is_some() { self.next_request_id } else { 0 },
            sample_rate,
            driver,
        };
        self.worker
            .send_weighted(
                request.clone(),
                std::mem::size_of::<SoundCardRuntimeRequest>() + MAX_DRIVER_BYTES,
            )
            .map_err(|error| match error {
                PendingSendError::Full(_) => StartRequestError::Full,
                PendingSendError::Disconnected(_) => StartRequestError::Disconnected,
            })?;
        self.next_request_id = next;
        Ok(request)
    }

    pub(crate) fn poll(&self) -> SoundCardRuntimeWorkerPoll {
        let mut started = Vec::with_capacity(1);
        let drain = self.worker.drain_events(&mut started, NonZeroUsize::MIN);
        match started.pop() {
            Some(started) => SoundCardRuntimeWorkerPoll::Started(started),
            None if drain.state == PendingDrainState::Disconnected => SoundCardRuntimeWorkerPoll::Disconnected,
            None => SoundCardRuntimeWorkerPoll::Pending,
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
}

fn bounded_error(error: &str) -> String {
    let mut end = error.len().min(MAX_START_ERROR_BYTES);
    while !error.is_char_boundary(end) {
        end -= 1;
    }
    error[..end].to_owned()
}

impl fmt::Debug for SoundCardRuntimeWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoundCardRuntimeWorker")
            .field("next_request_id", &self.next_request_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
