//! A ready signal reserves one presentation attempt. A busy/occluded Present did not consume
//! that reservation, so retry/coalesce the latest frame without waiting for another signal.

use crate::cadence::Submission;
use serde::Serialize;

pub const MAXIMUM_FRAME_LATENCY: u32 = 1;

#[derive(Clone, Default, Serialize)]
pub struct Counts {
    pub readiness_polls: u64,
    pub ready_signals: u64,
    pub not_ready: u64,
    pub reservation_retries: u64,
    pub accepted_presents: u64,
    pub busy_presents: u64,
    pub occluded_presents: u64,
}

#[derive(Clone, Default, Serialize)]
pub struct Report {
    pub all: Counts,
    // A warmup signal may be retained into sampling; its later retry/outcome is sampled,
    // even though acquiring the original signal was not.
    pub sampled: Counts,
}

#[derive(Default)]
pub struct Admission {
    reserved: bool,
    report: Report,
}

impl Admission {
    pub fn ready<E>(&mut self, collecting: bool, poll: impl FnOnce() -> Result<bool, E>) -> Result<bool, E> {
        if self.reserved {
            self.record(collecting, |counts| counts.reservation_retries += 1);
            return Ok(true);
        }
        self.record(collecting, |counts| counts.readiness_polls += 1);
        let ready = poll()?;
        if ready {
            self.record(collecting, |counts| counts.ready_signals += 1);
            self.reserved = true;
        } else {
            self.record(collecting, |counts| counts.not_ready += 1);
        }
        Ok(ready)
    }

    pub fn presented(&mut self, collecting: bool, result: Submission) {
        assert!(self.reserved, "Present requires a ready reservation");
        match result {
            Submission::Accepted => {
                self.record(collecting, |counts| counts.accepted_presents += 1);
                self.reserved = false;
            }
            Submission::Busy => self.record(collecting, |counts| counts.busy_presents += 1),
            Submission::Occluded => self.record(collecting, |counts| counts.occluded_presents += 1),
            Submission::NoTarget => unreachable!("no target cannot issue Present"),
        }
    }

    fn record(&mut self, collecting: bool, update: impl Fn(&mut Counts)) {
        update(&mut self.report.all);
        if collecting {
            update(&mut self.report.sampled);
        }
    }

    pub fn report(&self) -> Report {
        self.report.clone()
    }
}

#[cfg(test)]
mod tests;
