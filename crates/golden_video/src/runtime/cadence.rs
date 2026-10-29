use golden_time::{MonotonicTime, RationalRate, Rounding, TimeError};
use uuid::Uuid;

/// Worker-local absolute frame schedule, independent of show transport rate.
pub(super) struct Cadence {
    rate: RationalRate,
    anchor: MonotonicTime,
    next_index: u64,
    identity: Option<Uuid>,
    last_submitted: Option<u64>,
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
            identity: None,
            last_submitted: None,
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

    pub(super) fn schedule(
        &mut self,
        tick: CadenceTick,
        observed: golden_time::TimeSnapshot,
    ) -> Result<super::FrameSchedule, TimeError> {
        let slot_end = self.scheduled_at(tick.index.checked_add(1).ok_or(TimeError::Overflow)?)?;
        let cadence = *self.identity.get_or_insert_with(Uuid::new_v4);
        Ok(super::FrameSchedule {
            cadence,
            anchor: self.anchor,
            rate: self.rate,
            frame_index: tick.index,
            slot_start: tick.scheduled,
            slot_end,
            observed,
            coalesced_slots: tick.missed,
            skipped_slots: self
                .last_submitted
                .map_or(tick.index, |previous| tick.index - previous - 1),
        })
    }

    pub(super) fn submitted(&mut self, index: u64) {
        self.last_submitted = Some(index);
    }

    pub(super) fn fence(
        &mut self,
        observed: golden_time::TimeSnapshot,
    ) -> Result<Option<super::FrameSchedule>, TimeError> {
        if self.identity.is_none() {
            return Ok(None);
        }
        let tick = CadenceTick {
            index: self.next_index,
            scheduled: self.deadline()?,
            missed: 0,
        };
        let schedule = self.schedule(tick, observed)?;
        self.identity = None;
        Ok(Some(schedule))
    }
}

#[cfg(test)]
mod tests;
