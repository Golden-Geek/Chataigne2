//! A consumer's absolute phase belongs to its monotonic authority, not show transport.
use crate::runtime::cadence::Cadence;
use golden_time::{RationalRate, TimeEpoch, TimeSnapshot};

pub(super) struct Pacing {
    rate: Option<RationalRate>,
    phase: Option<(TimeEpoch, Cadence)>,
}

impl Pacing {
    pub fn new(rate: Option<RationalRate>) -> Self {
        Self { rate, phase: None }
    }
    pub fn due(&mut self, now: TimeSnapshot) -> bool {
        let Some(rate) = self.rate else {
            return true;
        };
        if self
            .phase
            .as_ref()
            .is_none_or(|(epoch, _)| !epoch.same_authority(now.epoch()))
        {
            self.phase = Some((now.epoch(), Cadence::new(rate, now.monotonic())));
        }
        self.phase
            .as_mut()
            .unwrap()
            .1
            .consume_due(now.monotonic())
            .is_ok_and(|tick| tick.is_some())
    }
    pub fn ready(&self, now: TimeSnapshot) -> bool {
        self.rate.is_none()
            || self.phase.as_ref().is_none_or(|(epoch, cadence)| {
                !epoch.same_authority(now.epoch())
                    || cadence.deadline().is_ok_and(|deadline| now.monotonic() >= deadline)
            })
    }
}

#[cfg(test)]
mod tests;
