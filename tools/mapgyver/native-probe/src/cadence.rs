//! CPU submission observations only. Accepted calls are not displayed or distinct frames.
use crate::timing::{Distribution, Series};
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Submission {
    Accepted,
    Busy,
    Occluded,
    NoTarget,
}

#[derive(Default, Serialize)]
pub struct Report {
    pub attempts: u64,
    pub accepted_calls: u64,
    pub busy: u64,
    pub occluded: u64,
    pub no_target: u64,
    pub late_attempts: u64,
    pub accepted_call_intervals: Option<Distribution>,
    pub deadline_lateness: Option<Distribution>,
}

#[derive(Default)]
pub struct Cadence {
    report: Report,
    previous: Option<Instant>,
    intervals: Series,
    lateness: Series,
}

impl Cadence {
    pub fn record(&mut self, now: Instant, deadline: Instant, collect: bool, outcome: Submission) {
        if !collect {
            self.previous = None;
            return;
        }
        self.report.attempts += 1;
        if now > deadline {
            self.report.late_attempts += 1;
        }
        self.lateness
            .push(now.saturating_duration_since(deadline).as_secs_f64() * 1000.0);
        match outcome {
            Submission::Accepted => {
                self.report.accepted_calls += 1;
                if let Some(previous) = self.previous.replace(now) {
                    self.intervals.push(now.duration_since(previous).as_secs_f64() * 1000.0);
                }
            }
            Submission::Busy => self.report.busy += 1,
            Submission::Occluded => self.report.occluded += 1,
            Submission::NoTarget => self.report.no_target += 1,
        }
    }

    pub fn finish(mut self) -> Report {
        self.report.accepted_call_intervals = self.intervals.distribution();
        self.report.deadline_lateness = self.lateness.distribution();
        self.report
    }
}

#[cfg(test)]
mod tests;
