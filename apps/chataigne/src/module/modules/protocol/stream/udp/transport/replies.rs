use std::collections::VecDeque;

use super::{publish, StreamingWorkerEvent, UdpOutboundSend, WorkerContext, UDP_EVENT_BYTES};
use golden_io::PendingSendError;

const SOURCE_COUNT: usize = 3;
const SMALL_REPLY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReplySource {
    Input,
    Output,
    Interfaces,
}

impl ReplySource {
    fn limit(self) -> usize {
        match self {
            Self::Input => UDP_EVENT_BYTES,
            Self::Output => SMALL_REPLY_BYTES,
            Self::Interfaces => SMALL_REPLY_BYTES + std::mem::size_of::<StreamingWorkerEvent>(),
        }
    }
}

struct PendingReply {
    source: ReplySource,
    event: StreamingWorkerEvent,
    weight: usize,
}

/// One continuation per producer, ordered by creation. A paused input/interface producer
/// cannot gate native output; a pending output outcome pauses its own destination cursor.
/// There is no second unbounded queue and no cloning/rebuilding on capacity retries.
#[derive(Default)]
pub(super) struct NativeReplies {
    pending: VecDeque<PendingReply>,
}

impl NativeReplies {
    pub(super) fn permits(&self, source: ReplySource) -> bool {
        !self.pending.iter().any(|reply| reply.source == source)
    }

    pub(super) fn offer(
        &mut self,
        context: &WorkerContext<UdpOutboundSend, StreamingWorkerEvent>,
        source: ReplySource,
        event: StreamingWorkerEvent,
    ) -> bool {
        assert!(
            self.permits(source),
            "pause a producer until its previous reply is admitted"
        );
        let event = if event.weight() > source.limit() {
            StreamingWorkerEvent::Error(format!(
                "UDP {source:?} reply exceeds its {}-byte limit; result rejected and native work is not replayed",
                source.limit(),
            ))
        } else {
            event
        };
        let weight = event.weight();
        let event = if self.pending.is_empty() {
            match context.try_publish_weighted(event, weight) {
                Ok(()) => return true,
                Err(PendingSendError::Full(event)) => event,
                Err(PendingSendError::Disconnected(_)) => return false,
            }
        } else {
            event
        };
        self.pending.push_back(PendingReply { source, event, weight });
        debug_assert!(self.pending.len() <= SOURCE_COUNT);
        true
    }

    pub(super) fn flush(&mut self, context: &WorkerContext<UdpOutboundSend, StreamingWorkerEvent>) -> bool {
        for _ in 0..SOURCE_COUNT {
            let Some(reply) = self.pending.pop_front() else { break };
            match context.try_publish_weighted(reply.event, reply.weight) {
                Ok(()) => {}
                Err(PendingSendError::Full(event)) => {
                    self.pending.push_front(PendingReply { event, ..reply });
                    break;
                }
                Err(PendingSendError::Disconnected(_)) => return false,
            }
        }
        true
    }

    /// Only retirement may wait for replies: every admitted native send is already done.
    /// Cancellation still interrupts the wait and abandons these explicitly cancelled results.
    pub(super) fn finish(self, context: &WorkerContext<UdpOutboundSend, StreamingWorkerEvent>) -> bool {
        self.pending.into_iter().all(|reply| publish(context, reply.event))
    }
}
