//! A ready signal reserves one presentation attempt. A busy/occluded Present did not consume
//! that reservation, so retry/coalesce the latest frame without waiting for another signal.

use crate::cadence::Submission;
use serde::Serialize;

pub const MAXIMUM_FRAME_LATENCY: u32 = 1;

#[derive(Clone, Default, Serialize)]
pub struct Report {
    pub readiness_polls: u64,
    pub ready_signals: u64,
    pub not_ready: u64,
    pub reservation_retries: u64,
    pub accepted_presents: u64,
    pub busy_presents: u64,
    pub occluded_presents: u64,
}

#[derive(Default)]
pub struct Admission {
    reserved: bool,
    report: Report,
}

impl Admission {
    pub fn ready<E>(&mut self, poll: impl FnOnce() -> Result<bool, E>) -> Result<bool, E> {
        if self.reserved {
            self.report.reservation_retries += 1;
            return Ok(true);
        }
        self.report.readiness_polls += 1;
        let ready = poll()?;
        if ready {
            self.report.ready_signals += 1;
            self.reserved = true;
        } else {
            self.report.not_ready += 1;
        }
        Ok(ready)
    }

    pub fn presented(&mut self, result: Submission) {
        assert!(self.reserved, "Present requires a ready reservation");
        match result {
            Submission::Accepted => {
                self.report.accepted_presents += 1;
                self.reserved = false;
            }
            Submission::Busy => self.report.busy_presents += 1,
            Submission::Occluded => self.report.occluded_presents += 1,
            Submission::NoTarget => unreachable!("no target cannot issue Present"),
        }
    }

    pub fn report(&self) -> Report {
        self.report.clone()
    }
}

#[cfg(test)]
mod tests;
