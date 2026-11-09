//! Bounded presentation windows. Producers prepare ahead; consumers select by one clock position.
use golden_time::{ShowPosition, TimeEpoch};

/// One source timeline observation, independent of its renderer's clock origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePosition {
    /// Current authority/discontinuity fence.
    pub epoch: TimeEpoch,
    /// Absolute presentation coordinate, including loop cycles.
    pub position: ShowPosition,
    /// Direction used only to retire windows already passed by this clock.
    pub reverse: bool,
}

/// A decoded image's half-open presentation interval and original normalized PTS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameWindow {
    epoch: TimeEpoch,
    start: ShowPosition,
    end: Option<ShowPosition>,
    pts: ShowPosition,
}
impl FrameWindow {
    /// Reject an empty interval; None end holds a final frame on a non-looping timeline.
    pub fn new(epoch: TimeEpoch, start: ShowPosition, end: Option<ShowPosition>, pts: ShowPosition) -> Option<Self> {
        end.is_none_or(|end| end > start)
            .then_some(Self { epoch, start, end, pts })
    }
    /// Original source PTS normalized to nanoseconds, before any loop offset.
    pub const fn pts(self) -> ShowPosition {
        self.pts
    }
    /// First included absolute presentation position.
    pub const fn start(self) -> ShowPosition {
        self.start
    }
    /// First excluded position, or an unbounded final hold.
    pub const fn end(self) -> Option<ShowPosition> {
        self.end
    }
    /// Whether this frame belongs to the observation's epoch and interval.
    pub fn contains(self, now: FramePosition) -> bool {
        self.epoch == now.epoch && self.start <= now.position && self.end.is_none_or(|end| now.position < end)
    }
    fn passed(self, now: FramePosition) -> bool {
        self.epoch != now.epoch
            || if now.reverse {
                self.start > now.position
            } else {
                self.end.is_some_and(|end| end <= now.position)
            }
    }
}

/// Exactly two retained images/observations. Full admission returns ownership to the caller.
pub struct FrameBuffer<T> {
    entries: [Option<(FrameWindow, T)>; 2],
}
impl<T> Default for FrameBuffer<T> {
    fn default() -> Self {
        Self { entries: [None, None] }
    }
}
impl<T> FrameBuffer<T> {
    /// Publish one current or future interval. Displaced values must be dropped outside owner locks.
    pub fn publish(&mut self, window: FrameWindow, value: T, now: FramePosition) -> Result<[Option<T>; 2], T> {
        if window.epoch != now.epoch {
            return Err(value);
        }
        let replace = self
            .entries
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|(old, _)| *old == window));
        let slot = replace.or_else(|| {
            self.entries
                .iter()
                .position(|entry| entry.as_ref().is_none_or(|(window, _)| window.passed(now)))
        });
        let Some(slot) = slot else {
            return Err(value);
        };
        let mut retired = [None, None];
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if index == slot || entry.as_ref().is_some_and(|(old, _)| old.passed(now)) {
                retired[index] = entry.take().map(|(_, value)| value);
            }
        }
        self.entries[slot] = Some((window, value));
        Ok(retired)
    }
    /// Select only a due frame. A gap is not permission to present a future image.
    pub fn sample(&self, now: FramePosition) -> Option<(FrameWindow, &T)> {
        self.entries
            .iter()
            .flatten()
            .find(|(window, _)| window.contains(now))
            .map(|(window, value)| (*window, value))
    }
    /// Whether no image has been scheduled.
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(Option::is_none)
    }
    /// Withdraw all windows and return their owned values for deferred destruction.
    pub fn clear(&mut self) -> [Option<T>; 2] {
        std::array::from_fn(|index| self.entries[index].take().map(|(_, value)| value))
    }
}

#[cfg(test)]
mod tests;
