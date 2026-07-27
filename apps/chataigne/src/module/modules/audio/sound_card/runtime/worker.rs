use std::{
    fmt,
    sync::mpsc::{SendError, TryRecvError},
    thread,
};

use golden_audio::SampleRate;
use golden_io::{PendingReceiver, WorkerTask, pending_channel};

use super::SoundCardRuntime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SoundCardRuntimeRequest {
    id: u64,
    sample_rate: SampleRate,
}

impl SoundCardRuntimeRequest {
    pub(crate) const fn sample_rate(self) -> SampleRate {
        self.sample_rate
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
    Start {
        request: SoundCardRuntimeRequest,
    },
    Retire(Box<SoundCardRuntime>),
}

pub(crate) struct SoundCardRuntimeWorker {
    task: WorkerTask<SoundCardRuntimeWorkerCommand>,
    events: Option<PendingReceiver<SoundCardRuntimeStarted>>,
    next_request_id: u64,
}

impl SoundCardRuntimeWorker {
    pub(crate) fn spawn() -> Result<Self, String> {
        Self::spawn_using(SoundCardRuntime::start)
    }

    #[cfg(test)]
    pub(crate) fn spawn_with<F>(starter: F) -> Result<Self, String>
    where
        F: Fn(SampleRate) -> Result<SoundCardRuntime, String> + Send + 'static,
    {
        Self::spawn_using(starter)
    }

    fn spawn_using<F>(starter: F) -> Result<Self, String>
    where
        F: Fn(SampleRate) -> Result<SoundCardRuntime, String> + Send + 'static,
    {
        let (event_sender, events) = pending_channel();
        let task = WorkerTask::spawn("chataigne-sound-card-runtime", move |commands| {
            while let Ok(command) = commands.recv() {
                match command {
                    SoundCardRuntimeWorkerCommand::Start { request } => {
                        let result = starter(request.sample_rate).map(Box::new);
                        let _ = event_sender.send(SoundCardRuntimeStarted {
                            request,
                            result,
                        });
                    }
                    SoundCardRuntimeWorkerCommand::Retire(mut runtime) => {
                        runtime.stop();
                    }
                }
            }
        })
        .map_err(|error| format!("failed to start Sound Card runtime worker: {error}"))?;
        Ok(Self {
            task,
            events: Some(events),
            next_request_id: 1,
        })
    }

    pub(crate) fn request_start(
        &mut self,
        sample_rate: SampleRate,
    ) -> Result<SoundCardRuntimeRequest, String> {
        let request = SoundCardRuntimeRequest {
            id: self.next_request_id,
            sample_rate,
        };
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.task
            .send(SoundCardRuntimeWorkerCommand::Start { request })
            .map_err(|_| "Sound Card runtime worker stopped".to_owned())?;
        Ok(request)
    }

    pub(crate) fn poll(&self) -> SoundCardRuntimeWorkerPoll {
        let Some(events) = self.events.as_ref() else {
            return SoundCardRuntimeWorkerPoll::Disconnected;
        };
        if events.has_pending() {
            events.clear_pending();
        }
        match events.try_recv() {
            Ok(started) => SoundCardRuntimeWorkerPoll::Started(started),
            Err(TryRecvError::Empty) => SoundCardRuntimeWorkerPoll::Pending,
            Err(TryRecvError::Disconnected) => SoundCardRuntimeWorkerPoll::Disconnected,
        }
    }

    pub(crate) fn retire(&self, runtime: SoundCardRuntime) {
        if let Err(SendError(SoundCardRuntimeWorkerCommand::Retire(runtime))) =
            self.task
                .send(SoundCardRuntimeWorkerCommand::Retire(Box::new(runtime)))
        {
            retire_detached(*runtime);
        }
    }
}

impl Drop for SoundCardRuntimeWorker {
    fn drop(&mut self) {
        let Some(events) = self.events.take() else {
            return;
        };
        let _ = thread::Builder::new()
            .name("chataigne-sound-card-result-retirement".to_owned())
            .spawn(move || drop(events));
    }
}

impl fmt::Debug for SoundCardRuntimeWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoundCardRuntimeWorker")
            .field("running", &self.task.is_running())
            .field("next_request_id", &self.next_request_id)
            .finish_non_exhaustive()
    }
}

pub(crate) fn retire_detached(mut runtime: SoundCardRuntime) {
    let _ = thread::Builder::new()
        .name("chataigne-sound-card-retirement".to_owned())
        .spawn(move || runtime.stop());
}
