use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    thread::{self, JoinHandle},
};

use golden_audio::{AudioEvent, AudioEventReceiver};
use golden_io::{PendingReceiver, pending_channel};

use super::RuntimeWakeSender;

pub(super) struct SoundCardRuntimeEvents {
    pending: PendingReceiver<AudioEvent>,
    refresh_rate_hz: Arc<AtomicU32>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SoundCardRuntimeEvents {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SoundCardRuntimeEvents")
            .field("refresh_rate_hz", &self.refresh_rate_hz.load(Ordering::Acquire))
            .field("worker_running", &self.worker.is_some())
            .finish_non_exhaustive()
    }
}

impl SoundCardRuntimeEvents {
    pub(super) fn spawn(
        events: AudioEventReceiver,
        wake: RuntimeWakeSender,
        refresh_rate_hz: u32,
    ) -> Result<Self, String> {
        let (pending_sender, pending) = pending_channel();
        let refresh_rate_hz = Arc::new(AtomicU32::new(
            super::bounded_levels_update_rate_hz(refresh_rate_hz),
        ));
        let worker_refresh_rate_hz = Arc::clone(&refresh_rate_hz);
        let worker = thread::Builder::new()
            .name("chataigne-sound-card-events".to_owned())
            .spawn(move || {
                loop {
                    let interval = super::levels_update_interval(
                        worker_refresh_rate_hz.load(Ordering::Acquire),
                    );
                    match events.recv_timeout(interval) {
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
            refresh_rate_hz,
            worker: Some(worker),
        })
    }

    pub(super) fn set_refresh_rate_hz(&self, refresh_rate_hz: u32) {
        self.refresh_rate_hz.store(
            super::bounded_levels_update_rate_hz(refresh_rate_hz),
            Ordering::Release,
        );
    }

    pub(super) fn drain(&self) -> Vec<AudioEvent> {
        let mut events = Vec::new();
        self.pending.drain_into(&mut events, NonZeroUsize::MAX);
        events
    }

    pub(super) fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
