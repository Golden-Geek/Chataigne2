use std::collections::VecDeque;

use golden_io::{PendingSendError, WorkerContext};

pub(crate) trait NativeReply: Sized {
    type Source: Copy + Eq;
    const SOURCES: usize;
    fn weight(&self) -> usize;
    fn limit(source: Self::Source) -> usize;
    fn oversized(source: Self::Source) -> Self;
}

struct PendingReply<E: NativeReply> {
    source: E::Source,
    event: E,
    weight: usize,
}

/// One bounded continuation per producer, ordered by creation. The owner pauses that
/// producer while its result is pending and can keep servicing independent native work.
/// Capacity retries retain the original event without cloning or rebuilding it.
pub(crate) struct NativeReplies<E: NativeReply> {
    pending: VecDeque<PendingReply<E>>,
}

impl<E: NativeReply> Default for NativeReplies<E> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }
}

impl<E: NativeReply> NativeReplies<E> {
    pub(crate) fn permits(&self, source: E::Source) -> bool {
        !self.pending.iter().any(|reply| reply.source == source)
    }

    pub(crate) fn offer<C>(&mut self, context: &WorkerContext<C, E>, source: E::Source, event: E) -> bool {
        assert!(
            self.permits(source),
            "pause a producer until its previous reply is admitted"
        );
        let event = if event.weight() > E::limit(source) {
            E::oversized(source)
        } else {
            event
        };
        let weight = event.weight();
        assert!(
            weight <= E::limit(source),
            "oversize feedback must fit its source reservation"
        );
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
        debug_assert!(self.pending.len() <= E::SOURCES);
        true
    }

    pub(crate) fn flush<C>(&mut self, context: &WorkerContext<C, E>) -> bool {
        for _ in 0..E::SOURCES {
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
    pub(crate) fn finish<C>(self, context: &WorkerContext<C, E>) -> bool {
        self.pending
            .into_iter()
            .all(|reply| context.publish_weighted(reply.event, reply.weight).is_ok())
    }
}
