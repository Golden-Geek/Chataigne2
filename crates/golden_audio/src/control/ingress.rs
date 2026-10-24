use std::{
    sync::{
        Arc, Mutex, OnceLock, TryLockError,
        atomic::{AtomicU8, Ordering},
    },
    thread::{self, Thread},
};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::{AudioCommand, AudioError, AudioErrorCategory, CommandSequence, QueuePressureCounters};

pub(super) const STATE_RUNNING: u8 = 0;
const STATE_STOPPING: u8 = 1;
pub(super) const STATE_STOPPED: u8 = 2;

#[derive(Debug)]
pub(super) struct CommandEnvelope {
    pub sequence: CommandSequence,
    pub command: AudioCommand,
}

#[derive(Debug)]
struct ShutdownAdmission {
    sequence: CommandSequence,
    queued: bool,
}

#[derive(Debug)]
struct Admission {
    queue: Producer<CommandEnvelope>,
    next_sequence: u64,
    shutdown: Option<ShutdownAdmission>,
}

#[derive(Clone, Debug)]
pub(super) struct CommandQueueProducer {
    producer: Arc<Mutex<Admission>>,
    state: Arc<AtomicU8>,
    worker_thread: Arc<OnceLock<Thread>>,
    pressure: QueuePressureCounters,
}

impl CommandQueueProducer {
    pub fn state(&self) -> Arc<AtomicU8> {
        self.state.clone()
    }
    pub fn accepting(&self) -> bool {
        self.state.load(Ordering::Acquire) == STATE_RUNNING
    }

    /// Sequence allocation and publication share one admission boundary. A caller
    /// cannot reserve a number, pause, and publish behind a newer command.
    pub fn try_push(&self, command: AudioCommand) -> Result<CommandSequence, AudioError> {
        let shutdown = matches!(command, AudioCommand::Shutdown);
        if !shutdown && !self.accepting() {
            return Err(AudioError::shutting_down());
        }
        let mut admission = match self.producer.try_lock() {
            Ok(producer) => producer,
            Err(TryLockError::WouldBlock) => {
                self.pressure.command_full();
                return Err(AudioError::queue_full("command_producer"));
            }
            Err(TryLockError::Poisoned(_)) => return Err(poisoned()),
        };
        if shutdown && let Some(existing) = &admission.shutdown {
            if !existing.queued && admission.queue.is_abandoned() {
                self.state.store(STATE_STOPPED, Ordering::Release);
                return Err(AudioError::shutting_down());
            }
            return Ok(existing.sequence);
        }
        self.check_open(&admission)?;
        if admission.queue.slots() == 0 {
            self.pressure.command_full();
            return Err(AudioError::queue_full("command"));
        }
        let sequence = next_sequence(&mut admission, shutdown)?;
        if shutdown {
            admission.shutdown = Some(ShutdownAdmission { sequence, queued: true });
            // Publish the closed state before the consumer can finish shutdown.
            self.state.store(STATE_STOPPING, Ordering::Release);
        }
        admission
            .queue
            .push(CommandEnvelope { sequence, command })
            .expect("the sole locked producer preflighted an available queue slot");
        self.wake_worker();
        Ok(sequence)
    }

    /// Forced host shutdown closes admission before waiting for queue capacity.
    /// Its retained barrier follows every accepted command and can be observed
    /// idempotently by other shutdown callers while the host owns this wait.
    pub fn shutdown_blocking(&self) -> Result<CommandSequence, AudioError> {
        loop {
            let mut admission = self.producer.lock().map_err(|_| poisoned())?;
            if let Some(existing) = &admission.shutdown {
                if existing.queued {
                    return Ok(existing.sequence);
                }
            } else {
                self.check_open(&admission)?;
                let sequence = next_sequence(&mut admission, true)?;
                admission.shutdown = Some(ShutdownAdmission {
                    sequence,
                    queued: false,
                });
                self.state.store(STATE_STOPPING, Ordering::Release);
            }
            if admission.queue.is_abandoned() {
                self.state.store(STATE_STOPPED, Ordering::Release);
                return Err(AudioError::shutting_down());
            }
            if admission.queue.slots() > 0 {
                let sequence = admission.shutdown.as_ref().unwrap().sequence;
                admission
                    .queue
                    .push(CommandEnvelope {
                        sequence,
                        command: AudioCommand::Shutdown,
                    })
                    .expect("the sole locked producer preflighted an available queue slot");
                admission.shutdown.as_mut().unwrap().queued = true;
                self.wake_worker();
                return Ok(sequence);
            }
            drop(admission);
            self.wake_worker();
            thread::yield_now();
        }
    }

    fn check_open(&self, admission: &Admission) -> Result<(), AudioError> {
        if !self.accepting() || admission.queue.is_abandoned() {
            if admission.queue.is_abandoned() {
                self.state.store(STATE_STOPPED, Ordering::Release);
            }
            return Err(AudioError::shutting_down());
        }
        Ok(())
    }
    fn wake_worker(&self) {
        if let Some(worker) = self.worker_thread.get() {
            worker.unpark();
        }
    }
    #[cfg(test)]
    pub(super) fn set_next_sequence_for_test(&self, sequence: CommandSequence) {
        self.producer.lock().unwrap().next_sequence = sequence.get();
    }
}

fn next_sequence(admission: &mut Admission, shutdown: bool) -> Result<CommandSequence, AudioError> {
    // The final number is reserved for shutdown: exhausting ordinary admission
    // must not strand a live worker or wrap a later command back to sequence one.
    if admission.next_sequence == u64::MAX && !shutdown {
        return Err(AudioError::new(
            AudioErrorCategory::InternalInvariant,
            "audio command sequence exhausted",
        ));
    }
    let sequence = CommandSequence::new(admission.next_sequence).expect("admission starts at one and never wraps");
    if !shutdown {
        admission.next_sequence += 1;
    }
    Ok(sequence)
}
fn poisoned() -> AudioError {
    AudioError::new(
        AudioErrorCategory::InternalInvariant,
        "audio command producer lock was poisoned",
    )
}

pub(super) fn command_queue(
    capacity: usize,
    pressure: QueuePressureCounters,
) -> (CommandQueueProducer, Consumer<CommandEnvelope>, Arc<OnceLock<Thread>>) {
    let (producer, consumer) = RingBuffer::new(capacity);
    let worker_thread = Arc::new(OnceLock::new());
    (
        CommandQueueProducer {
            producer: Arc::new(Mutex::new(Admission {
                queue: producer,
                next_sequence: CommandSequence::FIRST.get(),
                shutdown: None,
            })),
            state: Arc::new(AtomicU8::new(STATE_RUNNING)),
            worker_thread: worker_thread.clone(),
            pressure,
        },
        consumer,
        worker_thread,
    )
}
