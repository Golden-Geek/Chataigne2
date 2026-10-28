use golden_time::{MonotonicTime, RationalRate, Rounding, TimeError};

/// Worker-local absolute frame schedule, independent of show transport rate.
pub(super) struct Cadence {
    rate: RationalRate,
    anchor: MonotonicTime,
    next_index: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CadenceTick {
    pub(super) index: u64,
    pub(super) scheduled: MonotonicTime,
    pub(super) missed: u64,
}

impl Cadence {
    pub(super) fn new(rate: RationalRate, anchor: MonotonicTime) -> Self {
        Self {
            rate,
            anchor,
            next_index: 0,
        }
    }

    pub(super) fn deadline(&self) -> Result<MonotonicTime, TimeError> {
        self.scheduled_at(self.next_index)
    }

    pub(super) fn consume_due(&mut self, now: MonotonicTime) -> Result<Option<CadenceTick>, TimeError> {
        let elapsed = now.duration_since(self.anchor)?;
        if now < self.deadline()? {
            return Ok(None);
        }
        // Skip directly to the latest due ordinal; never run a catch-up frame loop.
        let index = self.rate.ticks_for(elapsed, Rounding::Floor)?;
        let next_index = index.checked_add(1).ok_or(TimeError::Overflow)?;
        let missed = index.checked_sub(self.next_index).ok_or(TimeError::Overflow)?;
        let scheduled = self.scheduled_at(index)?;
        self.next_index = next_index;
        Ok(Some(CadenceTick {
            index,
            scheduled,
            missed,
        }))
    }

    fn scheduled_at(&self, index: u64) -> Result<MonotonicTime, TimeError> {
        self.anchor.checked_add(self.rate.duration_for(index, Rounding::Ceil)?)
    }
}

#[cfg(test)]
mod tests;
