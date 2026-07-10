//! App-agnostic endpoint lifecycle, recovery, and bounded ingress primitives.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum IngressPolicy {
    Lossless,
    LatestWins,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryPolicy {
    pub initial_delay_ms: u64,
    pub maximum_delay_ms: u64,
    pub multiplier: u32,
}

impl RecoveryPolicy {
    pub fn validate(self) -> Result<Self, EndpointPolicyError> {
        if self.initial_delay_ms == 0 || self.maximum_delay_ms < self.initial_delay_ms || self.multiplier < 1 {
            return Err(EndpointPolicyError::InvalidRecoveryPolicy);
        }
        Ok(self)
    }
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self {
            initial_delay_ms: 250,
            maximum_delay_ms: 30_000,
            multiplier: 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EndpointPolicy {
    pub id: SmolStr,
    pub queue_capacity: usize,
    pub ingress: IngressPolicy,
    pub recovery: RecoveryPolicy,
}

impl EndpointPolicy {
    pub fn validate(&self) -> Result<(), EndpointPolicyError> {
        if self.id.is_empty() {
            return Err(EndpointPolicyError::EmptyId);
        }
        if self.queue_capacity == 0 {
            return Err(EndpointPolicyError::ZeroQueueCapacity);
        }
        self.recovery.validate()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting { attempt: u32 },
    Connected { since_ms: u64 },
    WaitingToRetry { attempt: u32, retry_at_ms: u64 },
    Stopped,
}

pub struct RecoveryStateMachine {
    policy: RecoveryPolicy,
    state: ConnectionState,
}

impl RecoveryStateMachine {
    pub fn new(policy: RecoveryPolicy) -> Result<Self, EndpointPolicyError> {
        Ok(Self {
            policy: policy.validate()?,
            state: ConnectionState::Disconnected,
        })
    }

    pub const fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn begin_connect(&mut self) -> bool {
        let attempt = match self.state {
            ConnectionState::Disconnected => 1,
            ConnectionState::WaitingToRetry { attempt, .. } => attempt,
            _ => return false,
        };
        self.state = ConnectionState::Connecting { attempt };
        true
    }

    pub fn connected(&mut self, now_ms: u64) {
        self.state = ConnectionState::Connected { since_ms: now_ms };
    }

    pub fn disconnected(&mut self, now_ms: u64) -> bool {
        let attempt = match self.state {
            ConnectionState::Connecting { attempt } => attempt,
            ConnectionState::Connected { .. } => 1,
            ConnectionState::Stopped => return false,
            ConnectionState::Disconnected | ConnectionState::WaitingToRetry { .. } => return false,
        };
        let exponent = attempt.saturating_sub(1).min(31);
        let delay = self
            .policy
            .initial_delay_ms
            .saturating_mul(u64::from(self.policy.multiplier).saturating_pow(exponent))
            .min(self.policy.maximum_delay_ms);
        self.state = ConnectionState::WaitingToRetry {
            attempt: attempt.saturating_add(1),
            retry_at_ms: now_ms.saturating_add(delay),
        };
        true
    }

    pub fn retry_due(&self, now_ms: u64) -> bool {
        matches!(self.state, ConnectionState::WaitingToRetry { retry_at_ms, .. } if now_ms >= retry_at_ms)
    }

    pub fn stop(&mut self) {
        self.state = ConnectionState::Stopped;
    }
}

pub struct BoundedIngress<T> {
    capacity: usize,
    policy: IngressPolicy,
    queue: VecDeque<T>,
}

impl<T> BoundedIngress<T> {
    pub fn new(capacity: usize, policy: IngressPolicy) -> Result<Self, EndpointPolicyError> {
        if capacity == 0 {
            return Err(EndpointPolicyError::ZeroQueueCapacity);
        }
        Ok(Self {
            capacity,
            policy,
            queue: VecDeque::with_capacity(capacity),
        })
    }

    pub fn push(&mut self, value: T) -> Result<(), IngressError> {
        if self.queue.len() == self.capacity {
            match self.policy {
                IngressPolicy::Lossless => return Err(IngressError::Full),
                IngressPolicy::LatestWins => {
                    self.queue.pop_front();
                }
            }
        }
        self.queue.push_back(value);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum EndpointPolicyError {
    #[error("endpoint identifier cannot be empty")]
    EmptyId,
    #[error("endpoint queue capacity must be non-zero")]
    ZeroQueueCapacity,
    #[error("recovery delays and multiplier are invalid")]
    InvalidRecoveryPolicy,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum IngressError {
    #[error("lossless ingress queue is full")]
    Full,
}

#[cfg(test)]
mod tests;
