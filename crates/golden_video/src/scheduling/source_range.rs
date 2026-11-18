//! Half-open source timestamp ranges preserve original PTS while presenting relative time.
use super::FrameWindow;
use golden_time::{ShowPosition, TimeEpoch};
use std::{ops::Range, time::Duration};

/// A nonempty portion of a source timeline with an inclusive start and exclusive end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceRange {
    start: Duration,
    end: Duration,
}

impl SourceRange {
    /// Admit positive ranges whose source timestamps fit the signed presentation clock.
    pub fn new(start: Duration, end: Duration) -> Option<Self> {
        (start < end && end.as_nanos() <= i64::MAX as u128).then_some(Self { start, end })
    }

    /// First source timestamp in the range.
    pub const fn start(self) -> Duration {
        self.start
    }

    /// First source timestamp after the range.
    pub const fn end(self) -> Duration {
        self.end
    }

    /// Length of the ranged timeline, including an exact endpoint for seeks and controls.
    pub fn duration(self) -> Duration {
        self.end - self.start
    }

    /// Membership in the source interval before adding an unwrapped loop offset.
    pub fn contains(self, timestamp: Duration) -> bool {
        timestamp >= self.start && timestamp < self.end
    }

    /// Map a relative clock position to its source coordinate, including the exact endpoint.
    pub fn source_position(self, relative: Duration) -> Duration {
        self.start + relative.min(self.duration())
    }

    /// Keep a decoder request in the range, so an exact endpoint displays the prior image.
    pub fn decode_position(self, relative: Duration) -> Duration {
        self.source_position(relative).min(self.end - Duration::from_nanos(1))
    }

    /// Map a source coordinate to the ranged timeline, clamping to its two endpoints.
    pub fn relative_position(self, timestamp: Duration) -> Duration {
        timestamp.saturating_sub(self.start).min(self.duration())
    }

    /// Clip a decoded interval to the active portion of a cycle while retaining the original PTS.
    /// With a missing next PTS, the last image fills the cycle. A terminal nonlooping image may
    /// have an open tail so the consumer can hold it as the clock reaches the endpoint.
    pub fn frame_window(
        self,
        epoch: TimeEpoch,
        origin_nanos: i128,
        active: Range<Duration>,
        pts: Duration,
        next: Option<Duration>,
        hold_final: bool,
    ) -> Option<FrameWindow> {
        if active.start >= active.end || active.end > self.duration() {
            return None;
        }
        let start = self.source_position(active.start).max(pts);
        let active_end = self.source_position(active.end);
        let end = next.map_or(active_end, |next| next.min(active_end));
        if start >= end {
            return None;
        }
        let coordinate = |source: Duration| {
            i64::try_from(origin_nanos.checked_add(self.relative_position(source).as_nanos() as i128)?)
                .ok()
                .map(ShowPosition::from_nanos)
        };
        let end = if hold_final && end == active_end {
            None
        } else {
            Some(coordinate(end)?)
        };
        FrameWindow::new(
            epoch,
            coordinate(start)?,
            end,
            ShowPosition::from_nanos(i64::try_from(pts.as_nanos()).ok()?),
        )
    }
}

#[cfg(test)]
mod tests;
