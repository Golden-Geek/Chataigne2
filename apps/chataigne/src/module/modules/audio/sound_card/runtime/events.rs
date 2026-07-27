use std::{
    sync::mpsc::TryRecvError,
    thread::{self, JoinHandle},
    time::Duration,
};

use golden_audio::{AudioEvent, AudioEventReceiver};
use golden_io::{PendingReceiver, pending_channel};

use super::RuntimeWakeSender;

/// Meter and analysis projection is UI-facing observation work, not engine-loop
/// scheduling. Five hertz keeps that projection responsive without recreating
/// the former 30 Hz main-thread poll.
const OBSERVATION_REFRESH_INTERVAL: Duration = Duration::from_millis(200);

pub(super) struct SoundCardRuntimeEvents {
    pending: PendingReceiver<AudioEvent>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SoundCardRuntimeEvents {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SoundCardRuntimeEvents")
            .field("worker_running", &self.worker.is_some())
            .finish_non_exhaustive()
    }
}

impl SoundCardRuntimeEvents {
    pub(super) fn spawn(events: AudioEventReceiver, wake: RuntimeWakeSender) -> Result<Self, String> {
        let (pending_sender, pending) = pending_channel();
        let worker = thread::Builder::new()
            .name("chataigne-sound-card-events".to_owned())
            .spawn(move || {
                loop {
                    match events.recv_timeout(OBSERVATION_REFRESH_INTERVAL) {
                        Ok(Some(event)) => {
                            if pending_sender.send(event).is_err() {
                                break;
                            }
                            wake.wake();
                        }
                        Ok(None) => wake.wake(),
                        Err(_) => break,
                    }
                }
            })
            .map_err(|error| format!("failed to start Sound Card event bridge: {error}"))?;
        Ok(Self {
            pending,
            worker: Some(worker),
        })
    }

    pub(super) fn drain(&self) -> Vec<AudioEvent> {
        self.pending.clear_pending();
        let mut events = Vec::new();
        loop {
            match self.pending.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
                    return events;
                }
            }
        }
    }

    pub(super) fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
