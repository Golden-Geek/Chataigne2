use std::{
    fmt,
    num::NonZeroUsize,
    sync::{
        mpsc::{RecvTimeoutError, TrySendError},
        OnceLock,
    },
    time::Duration,
};

use golden_audio::{BackendId, SampleRate};
use golden_io::{
    PendingDrainState, PendingReceiver, RetirementError, RetirementPermit, RetirementPool, WorkerTask,
    bounded_pending_channel,
};

use super::{RuntimeWakeSender, SoundCardRuntime};

const START_COALESCE_WINDOW: Duration = Duration::from_millis(10);
const LIFECYCLE_COMMAND_CAPACITY: usize = 8;
const LIFECYCLE_WORKER_CAPACITY: usize = 8;
const DETACHED_RUNTIME_RETIREMENT_CAPACITY: usize = 4;

fn lifecycle_worker_retirements() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| {
        RetirementPool::new(
            NonZeroUsize::new(LIFECYCLE_WORKER_CAPACITY)
                .expect("Sound Card lifecycle worker capacity is non-zero"),
        )
    })
}

fn detached_runtime_retirements() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| {
        RetirementPool::new(
            NonZeroUsize::new(DETACHED_RUNTIME_RETIREMENT_CAPACITY)
                .expect("Sound Card runtime retirement capacity is non-zero"),
        )
    })
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

enum SoundCardRuntimeWorkerCommand {
    Start { request: SoundCardRuntimeRequest },
    Retire(Box<SoundCardRuntime>),
}

pub(crate) struct SoundCardRuntimeWorker {
    task: Option<WorkerTask<SoundCardRuntimeWorkerCommand>>,
    events: Option<PendingReceiver<SoundCardRuntimeStarted>>,
    retirement_permit: Option<RetirementPermit>,
    next_request_id: u64,
}

impl SoundCardRuntimeWorker {
    pub(crate) fn spawn(wake: RuntimeWakeSender) -> Result<Self, String> {
        Self::spawn_using(wake, SoundCardRuntime::start_selected)
    }

    fn spawn_using<F>(wake: RuntimeWakeSender, starter: F) -> Result<Self, String>
    where
        F: Fn(SampleRate, Option<BackendId>) -> Result<SoundCardRuntime, String> + Send + 'static,
    {
        let retirement_permit = lifecycle_worker_retirements()
            .try_reserve()
            .map_err(|error| format!("Sound Card lifecycle worker rejected: {error}"))?;
        let (event_sender, events) = bounded_pending_channel(2, 2);
        let task = WorkerTask::spawn_with_capacity(
            "chataigne-sound-card-runtime",
            LIFECYCLE_COMMAND_CAPACITY,
            move |commands| {
                while let Ok(command) = commands.recv() {
                    match command {
                        SoundCardRuntimeWorkerCommand::Start { request: first_request } => {
                            // Driver construction may load native libraries and must never
                            // churn through a FIFO of stale UI selections. Wait for one
                            // short host-boundary window and construct only the newest
                            // request observed before initialization begins.
                            let mut request = first_request;
                            loop {
                                match commands.recv_timeout(START_COALESCE_WINDOW) {
                                    Ok(SoundCardRuntimeWorkerCommand::Start { request: newer }) => request = newer,
                                    Ok(SoundCardRuntimeWorkerCommand::Retire(mut runtime)) => runtime.stop(),
                                    Err(RecvTimeoutError::Timeout) => break,
                                    Err(RecvTimeoutError::Disconnected) => return,
                                }
                            }
                            let result = starter(request.sample_rate, request.driver.clone()).map(Box::new);
                            if event_sender.send(SoundCardRuntimeStarted { request, result }).is_ok() {
                                wake.wake();
                            }
                        }
                        SoundCardRuntimeWorkerCommand::Retire(mut runtime) => {
                            runtime.stop();
                        }
                    }
                }
            },
        )
        .map_err(|error| format!("failed to start Sound Card runtime worker: {error}"))?;
        Ok(Self {
            task: Some(task),
            events: Some(events),
            retirement_permit: Some(retirement_permit),
            next_request_id: 1,
        })
    }

    pub(crate) fn request_start_for_driver(
        &mut self,
        sample_rate: SampleRate,
        driver: Option<BackendId>,
    ) -> Result<SoundCardRuntimeRequest, String> {
        let request = SoundCardRuntimeRequest {
            id: self.next_request_id,
            sample_rate,
            driver,
        };
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.task
            .as_ref()
            .ok_or_else(|| "Sound Card runtime worker stopped".to_owned())?
            .send(SoundCardRuntimeWorkerCommand::Start {
                request: request.clone(),
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => "Sound Card runtime worker queue is full".to_owned(),
                TrySendError::Disconnected(_) => "Sound Card runtime worker stopped".to_owned(),
            })?;
        Ok(request)
    }

    pub(crate) fn poll(&self) -> SoundCardRuntimeWorkerPoll {
        let Some(events) = self.events.as_ref() else {
            return SoundCardRuntimeWorkerPoll::Disconnected;
        };
        let mut started = Vec::with_capacity(1);
        let drain = events.drain_into(&mut started, NonZeroUsize::MIN);
        match started.pop() {
            Some(started) => SoundCardRuntimeWorkerPoll::Started(started),
            None if drain.state == PendingDrainState::Disconnected => {
                SoundCardRuntimeWorkerPoll::Disconnected
            }
            None => SoundCardRuntimeWorkerPoll::Pending,
        }
    }

    pub(crate) fn retire(&self, runtime: SoundCardRuntime) -> Result<(), RetirementError<SoundCardRuntime>> {
        let Some(task) = self.task.as_ref() else {
            return retire_detached(runtime);
        };
        if let Err(
            TrySendError::Full(SoundCardRuntimeWorkerCommand::Retire(runtime))
            | TrySendError::Disconnected(SoundCardRuntimeWorkerCommand::Retire(runtime)),
        ) =
            task.send(SoundCardRuntimeWorkerCommand::Retire(Box::new(runtime)))
        {
            return retire_detached(*runtime);
        }
        Ok(())
    }
}

impl Drop for SoundCardRuntimeWorker {
    fn drop(&mut self) {
        let (Some(events), Some(task), Some(permit)) = (
            self.events.take(),
            self.task.take(),
            self.retirement_permit.take(),
        ) else {
            return;
        };
        let worker = task.into_join_handle();
        if let Err(error) = permit.spawn(
            "chataigne-sound-card-lifecycle-retirement",
            (events, worker),
            |(events, worker)| {
                drop(events);
                if let Some(worker) = worker {
                    let _ = worker.join();
                }
            },
        ) {
            eprintln!("Sound Card lifecycle retirement failed: {error}");
            let (events, worker) = error.into_value();
            drop(events);
            if let Some(worker) = worker {
                let _ = worker.join();
            }
        }
    }
}

impl fmt::Debug for SoundCardRuntimeWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoundCardRuntimeWorker")
            .field("running", &self.task.as_ref().is_some_and(WorkerTask::is_running))
            .field("next_request_id", &self.next_request_id)
            .finish_non_exhaustive()
    }
}

pub(crate) fn retire_detached(runtime: SoundCardRuntime) -> Result<(), RetirementError<SoundCardRuntime>> {
    detached_runtime_retirements().try_retire(
        "chataigne-sound-card-retirement",
        runtime,
        |mut runtime| runtime.stop(),
    )
}
