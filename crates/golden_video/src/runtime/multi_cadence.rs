//! Union of absolute deadlines: mixed rates never quantize onto the fastest output's grid.
use super::{
    FrameSchedule,
    cadence::{Cadence, CadenceTick},
};
use golden_time::{MonotonicTime, RationalRate, TimeError, TimeSnapshot};

pub(super) struct Cadences {
    clocks: Vec<Cadence>,
    due: Vec<Option<CadenceTick>>,
    selected: usize,
}

impl Cadences {
    pub fn rate(&self) -> RationalRate {
        self.clocks[self.selected].rate()
    }
    pub fn new(primary: RationalRate, additional: &[RationalRate], anchor: MonotonicTime) -> Self {
        let mut rates = vec![primary];
        for &rate in additional {
            if !rates.contains(&rate) {
                rates.push(rate);
            }
        }
        Self {
            due: vec![None; rates.len()],
            clocks: rates.into_iter().map(|rate| Cadence::new(rate, anchor)).collect(),
            selected: 0,
        }
    }
    pub fn deadline(&self) -> Result<MonotonicTime, TimeError> {
        self.clocks
            .iter()
            .map(Cadence::deadline)
            .try_fold(None, |earliest, next| {
                let next = next?;
                Ok(Some(earliest.map_or(next, |old: MonotonicTime| old.min(next))))
            })
            .map(|deadline| deadline.expect("primary cadence"))
    }
    pub fn consume_due(&mut self, now: MonotonicTime) -> Result<Option<CadenceTick>, TimeError> {
        for (clock, due) in self.clocks.iter_mut().zip(&mut self.due) {
            *due = clock.consume_due(now)?;
        }
        let selected = self
            .due
            .iter()
            .enumerate()
            .filter_map(|(i, tick)| tick.map(|tick| (i, tick)))
            .min_by_key(|(_, tick)| tick.scheduled);
        Ok(selected.map(|(index, tick)| {
            self.selected = index;
            tick
        }))
    }
    pub fn schedule(&mut self, tick: CadenceTick, observed: TimeSnapshot) -> Result<FrameSchedule, TimeError> {
        self.clocks[self.selected].schedule(tick, observed)
    }
    pub fn submitted(&mut self, _: u64) {
        for (clock, tick) in self.clocks.iter_mut().zip(&self.due) {
            if let Some(tick) = tick {
                clock.submitted(tick.index);
            }
        }
    }
    pub fn fences(&mut self, observed: TimeSnapshot) -> Result<Vec<FrameSchedule>, TimeError> {
        self.clocks
            .iter_mut()
            .filter_map(|clock| clock.fence(observed).transpose())
            .collect()
    }
}

#[cfg(test)]
mod tests;
