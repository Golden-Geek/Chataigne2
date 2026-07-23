use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::{AudioError, CommandSequence, GainDb};

use super::{QueuePressureCounters, assert_not_realtime};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GainMailboxTarget {
    Master,
    VirtualOutput(u16),
    InputPatchRoute(u32),
    MonitoringRoute(u32),
    PlaybackRoute(u32),
    OutputPatchRoute(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealtimeBarrierKind {
    PlanSwap,
    Play,
    Stop,
    StopAll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealtimeBarrier {
    pub sequence: CommandSequence,
    pub token: u64,
    pub kind: RealtimeBarrierKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RealtimeControlUpdate {
    Gain {
        sequence: CommandSequence,
        target: GainMailboxTarget,
        linear: f32,
    },
    Barrier(RealtimeBarrier),
}

#[derive(Debug)]
struct GainCell {
    version: AtomicU64,
    sequence: AtomicU64,
    required_barrier: AtomicU64,
    linear_bits: AtomicU32,
}

impl GainCell {
    fn new() -> Self {
        Self {
            version: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
            required_barrier: AtomicU64::new(0),
            linear_bits: AtomicU32::new(0.0_f32.to_bits()),
        }
    }

    fn write(&self, sequence: CommandSequence, required_barrier: u64, linear: f32) {
        let version = self.version.load(Ordering::Relaxed) & !1;
        self.version.store(version.wrapping_add(1), Ordering::Release);
        self.sequence.store(sequence.get(), Ordering::Relaxed);
        self.required_barrier.store(required_barrier, Ordering::Relaxed);
        self.linear_bits.store(linear.to_bits(), Ordering::Relaxed);
        self.version.store(version.wrapping_add(2), Ordering::Release);
    }

    #[inline]
    fn read(&self) -> Option<GainSnapshot> {
        for _ in 0..2 {
            let before = self.version.load(Ordering::Acquire);
            if before & 1 != 0 {
                continue;
            }
            let snapshot = GainSnapshot {
                sequence: self.sequence.load(Ordering::Relaxed),
                required_barrier: self.required_barrier.load(Ordering::Relaxed),
                linear: f32::from_bits(self.linear_bits.load(Ordering::Relaxed)),
            };
            let after = self.version.load(Ordering::Acquire);
            if before == after {
                return Some(snapshot);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct GainSnapshot {
    sequence: u64,
    required_barrier: u64,
    linear: f32,
}

#[derive(Debug)]
struct ControlMailbox {
    target: GainMailboxTarget,
    cell: Arc<GainCell>,
    current: GainSnapshot,
    dirty: bool,
}

#[derive(Clone, Copy, Debug)]
enum QueuedControl {
    Gain {
        sequence: CommandSequence,
        target: GainMailboxTarget,
        linear: f32,
    },
    Barrier(RealtimeBarrier),
}

#[derive(Debug)]
pub struct OrderedRealtimeControlWriter {
    mailboxes: Vec<ControlMailbox>,
    producer: Producer<QueuedControl>,
    last_barrier: u64,
    pressure: QueuePressureCounters,
}

impl OrderedRealtimeControlWriter {
    pub fn set_gain(
        &mut self,
        target: GainMailboxTarget,
        gain: GainDb,
        sequence: CommandSequence,
    ) -> Result<(), AudioError> {
        assert_not_realtime("gain mailbox publication");
        let Some(mailbox) = self.mailboxes.iter_mut().find(|mailbox| mailbox.target == target) else {
            return Err(AudioError::invalid_configuration(
                "gain target is not present in the compiled realtime control layout",
            ));
        };
        let snapshot = GainSnapshot {
            sequence: sequence.get(),
            required_barrier: self.last_barrier,
            linear: gain.to_linear(),
        };
        mailbox.current = snapshot;
        mailbox.dirty = true;
        mailbox.cell.write(sequence, snapshot.required_barrier, snapshot.linear);
        Ok(())
    }

    pub fn push_barrier(&mut self, barrier: RealtimeBarrier) -> Result<(), AudioError> {
        assert_not_realtime("realtime sequence barrier publication");
        if barrier.sequence.get() <= self.last_barrier {
            return Err(AudioError::invalid_configuration(
                "realtime sequence barriers must be strictly monotonic",
            ));
        }
        let required_slots = self.mailboxes.iter().filter(|mailbox| mailbox.dirty).count() + 1;
        if self.producer.slots() < required_slots {
            self.pressure.realtime_control_full();
            return Err(AudioError::queue_full("realtime_control"));
        }

        for mailbox in self.mailboxes.iter_mut().filter(|mailbox| mailbox.dirty) {
            let sequence = CommandSequence::new(mailbox.current.sequence)
                .expect("gain mailbox sequences are validated at publication");
            self.producer
                .push(QueuedControl::Gain {
                    sequence,
                    target: mailbox.target,
                    linear: mailbox.current.linear,
                })
                .expect("preflight reserved enough realtime control slots");
            mailbox.dirty = false;
        }
        self.producer
            .push(QueuedControl::Barrier(barrier))
            .expect("preflight reserved the realtime barrier slot");
        self.last_barrier = barrier.sequence.get();
        Ok(())
    }

    #[must_use]
    pub fn pressure(&self) -> super::QueuePressureSnapshot {
        self.pressure.snapshot()
    }
}

#[derive(Debug)]
struct RealtimeMailbox {
    target: GainMailboxTarget,
    cell: Arc<GainCell>,
    last_sequence: u64,
}

#[derive(Debug)]
pub struct OrderedRealtimeControlReader {
    mailboxes: Vec<RealtimeMailbox>,
    consumer: Consumer<QueuedControl>,
    processed_barrier: u64,
}

impl OrderedRealtimeControlReader {
    /// Emits queued barriers and the newest eligible gain values at a block boundary.
    #[inline]
    pub fn begin_block(&mut self, mut emit: impl FnMut(RealtimeControlUpdate)) {
        while let Ok(control) = self.consumer.pop() {
            match control {
                QueuedControl::Gain {
                    sequence,
                    target,
                    linear,
                } => {
                    if let Some(mailbox) = self.mailboxes.iter_mut().find(|mailbox| mailbox.target == target) {
                        mailbox.last_sequence = mailbox.last_sequence.max(sequence.get());
                    }
                    emit(RealtimeControlUpdate::Gain {
                        sequence,
                        target,
                        linear,
                    });
                }
                QueuedControl::Barrier(barrier) => {
                    self.processed_barrier = self.processed_barrier.max(barrier.sequence.get());
                    emit(RealtimeControlUpdate::Barrier(barrier));
                }
            }
        }

        for mailbox in &mut self.mailboxes {
            let Some(snapshot) = mailbox.cell.read() else {
                continue;
            };
            if snapshot.sequence <= mailbox.last_sequence || snapshot.required_barrier > self.processed_barrier {
                continue;
            }
            let Ok(sequence) = CommandSequence::new(snapshot.sequence) else {
                continue;
            };
            mailbox.last_sequence = snapshot.sequence;
            emit(RealtimeControlUpdate::Gain {
                sequence,
                target: mailbox.target,
                linear: snapshot.linear,
            });
        }
    }
}

pub fn ordered_realtime_controls(
    targets: impl IntoIterator<Item = GainMailboxTarget>,
    queue_capacity: usize,
) -> Result<(OrderedRealtimeControlWriter, OrderedRealtimeControlReader), AudioError> {
    if queue_capacity == 0 {
        return Err(AudioError::invalid_configuration(
            "realtime control queue capacity must be greater than zero",
        ));
    }
    let pressure = QueuePressureCounters::default();
    let mut control_mailboxes = Vec::new();
    let mut realtime_mailboxes = Vec::new();
    for target in targets {
        if control_mailboxes
            .iter()
            .any(|mailbox: &ControlMailbox| mailbox.target == target)
        {
            return Err(AudioError::invalid_configuration(
                "realtime gain mailbox targets must be unique",
            ));
        }
        let cell = Arc::new(GainCell::new());
        control_mailboxes.push(ControlMailbox {
            target,
            cell: Arc::clone(&cell),
            current: GainSnapshot {
                sequence: 0,
                required_barrier: 0,
                linear: 0.0,
            },
            dirty: false,
        });
        realtime_mailboxes.push(RealtimeMailbox {
            target,
            cell,
            last_sequence: 0,
        });
    }
    let (producer, consumer) = RingBuffer::new(queue_capacity);
    Ok((
        OrderedRealtimeControlWriter {
            mailboxes: control_mailboxes,
            producer,
            last_barrier: 0,
            pressure,
        },
        OrderedRealtimeControlReader {
            mailboxes: realtime_mailboxes,
            consumer,
            processed_barrier: 0,
        },
    ))
}
