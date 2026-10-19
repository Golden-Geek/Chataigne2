use super::transport::{self, HttpQueuedRequest, HttpTransportConfig, HttpTransportHandle, HttpWorkerEvent};
use golden_io::{PendingDrainState, RetirementPool};
use std::collections::VecDeque;
use std::num::NonZeroUsize;

pub(super) const MAX_HTTP_GENERATIONS: usize = 3;

pub(super) struct HttpTransports {
    generations: VecDeque<Generation>,
    lifetimes: RetirementPool,
    blocked: bool,
}

struct Generation {
    config: HttpTransportConfig,
    handle: HttpTransportHandle,
    accepting: bool,
}

pub(super) struct TransportPoll {
    pub events: Vec<HttpWorkerEvent>,
    pub active_ended: bool,
    pub active_response: bool,
}

impl Default for HttpTransports {
    fn default() -> Self {
        Self::with_lifetimes(transport::worker_lifetimes().clone())
    }
}

impl HttpTransports {
    pub(super) fn with_lifetimes(lifetimes: RetirementPool) -> Self {
        Self {
            generations: VecDeque::new(),
            lifetimes,
            blocked: true,
        }
    }

    pub(super) fn block_admission(&mut self) {
        self.blocked = true;
    }

    pub(super) fn configure(&mut self, config: HttpTransportConfig) -> Result<(), String> {
        self.configure_using(config, HttpTransportHandle::spawn_generation)
    }

    pub(super) fn configure_using<F>(&mut self, config: HttpTransportConfig, spawn: F) -> Result<(), String>
    where
        F: FnOnce(
            HttpTransportConfig,
            &RetirementPool,
            Option<transport::GenerationFence>,
        ) -> Result<HttpTransportHandle, String>,
    {
        if self
            .generations
            .back()
            .is_some_and(|last| last.accepting && last.config == config)
        {
            self.blocked = false;
            return Ok(());
        }
        self.blocked = true;
        if self.generations.len() >= MAX_HTTP_GENERATIONS {
            self.pause_admission();
            return Err("HTTP configuration is waiting for older admitted requests to finish".into());
        }
        let predecessor = self.generations.back().map(|last| last.handle.completion_fence());
        let next = match spawn(config.clone(), &self.lifetimes, predecessor) {
            Ok(handle) => handle,
            Err(error) => {
                // Keep every old owner. Closing idle admission also lets its native slot
                // retire, so a full pool can recover without allocating another worker.
                self.pause_admission();
                return Err(error);
            }
        };
        self.pause_admission();
        self.generations.push_back(Generation {
            config,
            handle: next,
            accepting: true,
        });
        self.blocked = false;
        Ok(())
    }

    pub(super) fn send(&self, request: HttpQueuedRequest) -> Result<(), String> {
        if self.blocked {
            return Err("HTTP configuration is pending; request was not admitted".into());
        }
        let active = self
            .generations
            .back()
            .filter(|last| last.accepting)
            .ok_or_else(|| "HTTP worker is not available".to_string())?;
        active.handle.send(request)
    }

    /// Disabling or invalid settings stop admission, not previously accepted commands.
    pub(super) fn pause_admission(&mut self) {
        self.blocked = true;
        if let Some(last) = self.generations.back_mut() {
            last.accepting = false;
            last.handle.finish_admission();
        }
    }

    /// Only destruction abandons delivery, because the destination itself is being removed.
    pub(super) fn cancel_all(&mut self) {
        self.blocked = true;
        self.generations.clear();
    }

    pub(super) fn has_pending(&self) -> bool {
        self.generations.front().is_some_and(|first| first.handle.has_pending())
    }

    pub(super) fn poll(&mut self) -> TransportPoll {
        let mut poll = TransportPoll {
            events: Vec::new(),
            active_ended: false,
            active_response: false,
        };
        // Both the generation count and total service budget are fixed. A slow predecessor
        // retains newer results at their bounded worker boundary rather than reordering them.
        while poll.events.len() < transport::HTTP_RESPONSE_CAPACITY {
            let Some(first) = self.generations.front() else {
                break;
            };
            let budget = NonZeroUsize::new(transport::HTTP_RESPONSE_CAPACITY - poll.events.len()).unwrap();
            let before = poll.events.len();
            // Observe completion before draining: observing it afterwards could race the
            // final publication and discard a result that arrived just after an empty poll.
            let requests_drained = first.handle.requests_drained();
            let drain = first.handle.drain_events(&mut poll.events, budget);
            poll.active_response |= first.accepting
                && poll.events[before..]
                    .iter()
                    .any(|event| matches!(event, HttpWorkerEvent::Response(_)));
            if drain.state != PendingDrainState::Disconnected
                && !(requests_drained && drain.state == PendingDrainState::Empty)
            {
                break;
            }
            let ended = self.generations.pop_front().expect("polled HTTP generation");
            if ended.accepting {
                self.blocked = true;
                poll.active_ended = true;
            }
        }
        poll
    }
}
