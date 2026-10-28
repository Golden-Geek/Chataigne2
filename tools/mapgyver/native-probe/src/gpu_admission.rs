//! Two bounded frame batches, independent of optional profiling queries.

use serde::Serialize;
use std::collections::VecDeque;

pub const CAPACITY: usize = 2;

#[derive(Debug, PartialEq, Eq)]
pub struct Reservation(usize);

impl Reservation {
    pub fn slot(&self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Window {
    pub attempts: u64,
    pub admitted: u64,
    pub busy: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Report {
    pub maximum_in_flight: usize,
    pub peak_in_flight: usize,
    pub submitted: u64,
    pub completed: u64,
    pub pending: usize,
    pub reserved: bool,
    pub warmup: Window,
    pub sampled: Window,
}

#[derive(Default)]
pub struct Admission {
    pending: VecDeque<usize>,
    reserved: Option<usize>,
    report: ReportState,
}

#[derive(Default)]
struct ReportState {
    peak: usize,
    submitted: u64,
    completed: u64,
    warmup: Window,
    sampled: Window,
}

impl Admission {
    pub fn poll<E>(&mut self, mut ready: impl FnMut(usize) -> Result<bool, E>) -> Result<(), E> {
        while let Some(&slot) = self.pending.front() {
            // One ordered immediate context: never retire a newer batch ahead of its prefix.
            if !ready(slot)? {
                break;
            }
            self.pending.pop_front();
            self.report.completed += 1;
        }
        Ok(())
    }

    pub fn reserve<E>(
        &mut self,
        collecting: bool,
        ready: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<Option<Reservation>, E> {
        assert!(
            self.reserved.is_none(),
            "a frame reservation must be submitted before another attempt"
        );
        self.poll(ready)?;
        let window = if collecting {
            &mut self.report.sampled
        } else {
            &mut self.report.warmup
        };
        window.attempts += 1;
        let Some(slot) = (0..CAPACITY).find(|slot| !self.pending.contains(slot)) else {
            window.busy += 1;
            return Ok(None);
        };
        window.admitted += 1;
        let reservation = Reservation(slot);
        self.reserved = Some(slot);
        self.report.peak = self.report.peak.max(self.pending.len() + 1);
        Ok(Some(reservation))
    }

    pub fn submitted(&mut self, reservation: Reservation) {
        assert_eq!(
            self.reserved.take(),
            Some(reservation.slot()),
            "submission must own the current reservation"
        );
        self.pending.push_back(reservation.slot());
        self.report.submitted += 1;
    }

    pub fn report(&self) -> Report {
        Report {
            maximum_in_flight: CAPACITY,
            peak_in_flight: self.report.peak,
            submitted: self.report.submitted,
            completed: self.report.completed,
            pending: self.pending.len(),
            reserved: self.reserved.is_some(),
            warmup: self.report.warmup,
            sampled: self.report.sampled,
        }
    }
}

#[cfg(test)]
mod tests;
