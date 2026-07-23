use std::{
    sync::{Arc, Mutex, OnceLock, TryLockError},
    thread::{self, Thread},
};

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::{AudioCommand, AudioError, AudioErrorCategory, CommandSequence, QueuePressureCounters};

#[derive(Debug)]
pub(super) struct CommandEnvelope {
    pub sequence: CommandSequence,
    pub command: AudioCommand,
}

#[derive(Clone, Debug)]
pub(super) struct CommandQueueProducer {
    producer: Arc<Mutex<Producer<CommandEnvelope>>>,
    worker_thread: Arc<OnceLock<Thread>>,
    pressure: QueuePressureCounters,
}

impl CommandQueueProducer {
    pub fn try_push(&self, envelope: CommandEnvelope) -> Result<(), AudioError> {
        let mut producer = match self.producer.try_lock() {
            Ok(producer) => producer,
            Err(TryLockError::WouldBlock) => {
                self.pressure.command_full();
                return Err(AudioError::queue_full("command_producer"));
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    "audio command producer lock was poisoned",
                ));
            }
        };
        match producer.push(envelope) {
            Ok(()) => {
                self.wake_worker();
                Ok(())
            }
            Err(PushError::Full(_)) => {
                self.pressure.command_full();
                Err(AudioError::queue_full("command"))
            }
        }
    }

    pub fn push_blocking(&self, mut envelope: CommandEnvelope) -> Result<(), AudioError> {
        loop {
            let result = self
                .producer
                .lock()
                .map_err(|_| {
                    AudioError::new(
                        AudioErrorCategory::InternalInvariant,
                        "audio command producer lock was poisoned",
                    )
                })?
                .push(envelope);
            match result {
                Ok(()) => {
                    self.wake_worker();
                    return Ok(());
                }
                Err(PushError::Full(returned)) => {
                    envelope = returned;
                    self.wake_worker();
                    thread::yield_now();
                }
            }
        }
    }

    fn wake_worker(&self) {
        if let Some(worker) = self.worker_thread.get() {
            worker.unpark();
        }
    }
}

pub(super) fn command_queue(
    capacity: usize,
    pressure: QueuePressureCounters,
) -> (CommandQueueProducer, Consumer<CommandEnvelope>, Arc<OnceLock<Thread>>) {
    let (producer, consumer) = RingBuffer::new(capacity);
    let worker_thread = Arc::new(OnceLock::new());
    (
        CommandQueueProducer {
            producer: Arc::new(Mutex::new(producer)),
            worker_thread: Arc::clone(&worker_thread),
            pressure,
        },
        consumer,
        worker_thread,
    )
}
