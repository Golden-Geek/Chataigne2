use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError},
        OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use golden_core::{edit::Edit, events::CustomEvent, node::NodeId};

pub(crate) const SOUND_CARD_RUNTIME_WAKE_TOPIC: &str =
    "chataigne.sound_card.runtime_wake.v1";
const DELAYED_WAKE_CAPACITY: usize = 64;

struct ScheduledWake {
    deadline: Instant,
    wake: RuntimeWakeSender,
}

struct DelayedWakeScheduler {
    sender: SyncSender<ScheduledWake>,
    pending: Arc<AtomicUsize>,
}

/// Coalesces worker notifications into transient engine events.
///
/// The host/runtime boundary owns wake timing. The Sound Card node itself remains
/// passive and only runs when an authored graph event or this wake event reaches
/// its inbox.
#[derive(Clone)]
pub(crate) struct RuntimeWakeSender {
    edits: Sender<Edit>,
    module: NodeId,
    pending: Arc<AtomicBool>,
}

impl RuntimeWakeSender {
    pub(crate) fn new(edits: Sender<Edit>, module: NodeId) -> Self {
        Self {
            edits,
            module,
            pending: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn wake(&self) {
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let event = CustomEvent::transient(
            SOUND_CARD_RUNTIME_WAKE_TOPIC,
            Some(self.module),
            serde_json::Value::Null,
        );
        if self.edits.send(Edit::EmitCustomEvent { event }).is_err() {
            self.pending.store(false, Ordering::Release);
        }
    }

    pub(crate) fn wake_after(&self, delay: Duration) {
        let Some(scheduler) = delayed_wake_scheduler() else {
            self.wake();
            return;
        };
        if scheduler
            .try_schedule(ScheduledWake {
                deadline: Instant::now() + delay,
                wake: self.clone(),
            })
            .is_err()
        {
            self.wake();
        }
    }

    pub(crate) fn acknowledge(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

impl DelayedWakeScheduler {
    fn try_schedule(&self, wake: ScheduledWake) -> Result<(), ScheduledWake> {
        if self
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                (pending < DELAYED_WAKE_CAPACITY).then_some(pending + 1)
            })
            .is_err()
        {
            return Err(wake);
        }
        match self.sender.try_send(wake) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(wake)) | Err(TrySendError::Disconnected(wake)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                Err(wake)
            }
        }
    }
}

fn delayed_wake_scheduler() -> Option<&'static DelayedWakeScheduler> {
    static SCHEDULER: OnceLock<Option<DelayedWakeScheduler>> = OnceLock::new();
    SCHEDULER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel(DELAYED_WAKE_CAPACITY);
            let pending = Arc::new(AtomicUsize::new(0));
            let worker_pending = pending.clone();
            match thread::Builder::new()
                .name("chataigne-sound-card-retry-timer".to_owned())
                .spawn(move || delayed_wake_loop(receiver, worker_pending))
            {
                Ok(_worker) => Some(DelayedWakeScheduler { sender, pending }),
                Err(error) => {
                    eprintln!("failed to start Sound Card retry timer: {error}");
                    None
                }
            }
        })
        .as_ref()
}

fn delayed_wake_loop(receiver: Receiver<ScheduledWake>, pending: Arc<AtomicUsize>) {
    let mut scheduled = Vec::<ScheduledWake>::with_capacity(DELAYED_WAKE_CAPACITY);
    loop {
        if scheduled.is_empty() {
            let Ok(wake) = receiver.recv() else {
                return;
            };
            scheduled.push(wake);
        } else {
            let now = Instant::now();
            let next_deadline = scheduled
                .iter()
                .map(|wake| wake.deadline)
                .min()
                .expect("non-empty delayed wake set has a deadline");
            match receiver.recv_timeout(next_deadline.saturating_duration_since(now)) {
                Ok(wake) => scheduled.push(wake),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        let now = Instant::now();
        let mut index = 0;
        while index < scheduled.len() {
            if scheduled[index].deadline <= now {
                scheduled.swap_remove(index).wake.wake();
                pending.fetch_sub(1, Ordering::AcqRel);
            } else {
                index += 1;
            }
        }
    }
}

impl std::fmt::Debug for RuntimeWakeSender {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeWakeSender")
            .field("module", &self.module)
            .field("pending", &self.pending.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}
