use std::time::{Duration, Instant};

use golden_io::ReconnectBackoff;
use windows::core::HRESULT;

use super::super::observation::OutputState;

pub(super) struct Recovery {
    pub state: OutputState,
    pub generation: u64,
    pub attempts: u64,
    pub error: Option<HRESULT>,
    retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
}

impl Recovery {
    pub fn new() -> Self {
        Self {
            state: OutputState::Starting,
            generation: 0,
            attempts: 0,
            error: None,
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(2)),
        }
    }
    pub fn begin_attempt(&mut self, now: Instant) -> bool {
        if matches!(
            self.state,
            OutputState::Stopping | OutputState::Stopped | OutputState::DeviceLost
        ) || self.retry_at.is_some_and(|at| at > now)
        {
            return false;
        }
        if self.generation == u64::MAX {
            self.state = OutputState::Stopped;
            return false;
        }
        self.attempts = self.attempts.saturating_add(1);
        true
    }
    pub fn created(&mut self) {
        self.generation += 1;
        self.state = OutputState::Ready;
        self.error = None;
        self.retry_at = None;
    }
    pub fn failed(&mut self, error: HRESULT, now: Instant) {
        self.state = OutputState::Recovering;
        self.error = Some(error);
        self.retry_at = Some(self.backoff.schedule(now));
    }
    pub fn accepted(&mut self) {
        self.state = OutputState::Ready;
        self.error = None;
        self.retry_at = None;
        self.backoff.reset();
    }
    pub fn device_lost(&mut self, error: HRESULT) {
        self.state = OutputState::DeviceLost;
        self.error = Some(error);
        self.retry_at = None;
    }
    pub fn stop(&mut self) {
        self.state = OutputState::Stopping;
        self.retry_at = None;
    }
}
