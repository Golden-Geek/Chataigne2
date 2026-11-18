#[cfg(feature = "video-files")]
use golden_time::MonotonicTime;
use golden_time::{Clock, PlaybackRate, ShowPosition, TimeAuthority, TimeEpoch, TimeError, TimeMapping};
use golden_video::scheduling::{LoopBlend, LoopTimeline, SourceRange};
use std::time::Duration;

/// One immutable media-wide observation. Every channel samples this same position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportSample {
    /// Authority identity and explicit-control discontinuity fence.
    pub epoch: TimeEpoch,
    /// Loop cycle within the epoch; a seek result from an old cycle must not publish.
    pub cycle: i64,
    /// Shared position relative to the crop, clipped or wrapped against its duration.
    pub position: Duration,
    /// Effective playback state, false at a non-looping endpoint.
    pub playing: bool,
    pub(crate) mapping: TimeMapping,
    pub(crate) duration: Duration,
    pub(crate) looping: bool,
    pub(super) source_range: Option<SourceRange>,
    pub(super) timeline: Option<LoopTimeline>,
    pub(super) reverse: bool,
}

impl TransportSample {
    pub(crate) fn has_finite_end(self) -> bool {
        !self.looping && !self.duration.is_zero()
    }

    /// Wall time until the directional endpoint of a finite moving transport. A completed
    /// clock still reports zero, while pause and looping provide no finite end signal.
    pub(crate) fn finite_end_delay(self) -> Option<Duration> {
        let rate = self.mapping.rate();
        if !self.has_finite_end() || rate.numerator() == 0 {
            return None;
        }
        let distance = if rate.numerator() < 0 {
            self.position
        } else {
            self.duration.saturating_sub(self.position)
        };
        let delay = (distance.as_nanos() * u128::from(rate.denominator()))
            .div_ceil(u128::from(rate.numerator().unsigned_abs()));
        u64::try_from(delay).ok().map(Duration::from_nanos)
    }

    pub(crate) fn frame_position(self) -> Result<golden_video::scheduling::FramePosition, String> {
        let raw = self.cycle_origin() + self.position.as_nanos() as i128;
        // Reverse looping maps the exact seam to the previous cycle's final image.
        let raw = raw - i128::from(self.playing && self.looping && self.reverse && self.position == self.cycle_end());
        Ok(golden_video::scheduling::FramePosition {
            epoch: self.epoch,
            position: ShowPosition::from_nanos(i64::try_from(raw).map_err(|_| "Video presentation overflow")?),
            reverse: self.mapping.rate().numerator() < 0,
        })
    }
    /// Shorter channels hold their last available frame until the whole media reaches its end.
    /// The decoder selects the frame at/before this timestamp; duration is not a frame number.
    pub fn channel_position(self, channel_duration: Duration) -> Duration {
        let position = if self.playing && self.reverse && self.position == self.cycle_end() {
            self.position.saturating_sub(Duration::from_nanos(1))
        } else {
            self.position
        };
        self.source_position(position, channel_duration)
    }
    pub(super) fn source_position(self, position: Duration, channel_duration: Duration) -> Duration {
        self.source_range.map_or(Duration::ZERO, |range| {
            range.decode_position(position).min(channel_duration)
        })
    }
    pub(super) fn cycle_origin(self) -> i128 {
        self.timeline.map_or_else(
            || i128::from(self.cycle) * self.duration.as_nanos() as i128,
            |timeline| timeline.origin_nanos(self.cycle),
        )
    }
    pub(super) fn cycle_start(self) -> Duration {
        self.timeline.map_or(Duration::ZERO, |timeline| {
            timeline.source_bounds(self.cycle, self.reverse).0
        })
    }
    pub(super) fn cycle_end(self) -> Duration {
        self.timeline.map_or(self.duration, |timeline| {
            timeline.source_bounds(self.cycle, self.reverse).1
        })
    }
    pub(super) fn loop_blend(self) -> Option<LoopBlend> {
        // The paused boundaries show their original in-range first/last frame. In
        // playback, the seam instead uses the configured overlap.
        if !self.playing && (self.position.is_zero() || self.position == self.duration) {
            return None;
        }
        self.timeline?.blend_at(self.position, self.reverse)
    }
    #[cfg(feature = "video-files")]
    pub(super) fn needs_loop_companion(self) -> bool {
        self.timeline.is_some() && (self.playing || self.loop_blend().is_some())
    }
    /// Project a bounded ahead sample against the same epoch, without observing a second clock.
    pub(super) fn at_presentation_position(self, position: ShowPosition) -> Option<Self> {
        let projected = self.timeline?.sample(position, self.reverse)?;
        Some(Self {
            cycle: projected.cycle,
            position: projected.position,
            ..self
        })
    }
}

/// Rejected transport control. Validation failures do not modify accepted state.
#[derive(Debug)]
pub enum TransportError {
    /// An unavailable media has no known timeline to play or seek.
    NoDuration,
    /// Position or duration is not representable by Golden's signed nanosecond timeline.
    Overflow,
    /// A seek/step timestamp is outside the admitted media duration.
    OutsideDuration,
    /// Monotonic observation or checked clock mapping failed.
    Time(TimeError),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDuration => formatter.write_str("Video duration is unavailable"),
            Self::Overflow => formatter.write_str("Video timeline exceeds signed nanoseconds"),
            Self::OutsideDuration => formatter.write_str("Video position is outside its duration"),
            Self::Time(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}
impl std::error::Error for TransportError {}
impl From<TimeError> for TransportError {
    fn from(value: TimeError) -> Self {
        Self::Time(value)
    }
}

/// Single app-owned transport per media, rather than a transport per channel or Reference.
/// Authoring/worker adapters provide typed intents; no file or device access occurs here.
pub struct MediaTransport<C: Clock> {
    authority: TimeAuthority<C>,
    source_range: Option<SourceRange>,
    rate: PlaybackRate,
    playing: bool,
    looping: bool,
    loop_overlap: Duration,
}

impl<C: Clock> MediaTransport<C> {
    /// New Video starts paused at zero, rate one and looping off.
    pub fn new(clock: C) -> Result<Self, TransportError> {
        Ok(Self {
            authority: TimeAuthority::new(clock, ShowPosition::from_nanos(0), PlaybackRate::PAUSED)?,
            source_range: None,
            rate: PlaybackRate::NORMAL,
            playing: false,
            looping: false,
            loop_overlap: Duration::ZERO,
        })
    }

    /// Current authored rate projection, retained independently of play/pause state.
    pub fn rate(&self) -> PlaybackRate {
        self.rate
    }

    /// Whether media-wide looping is enabled.
    pub fn looping(&self) -> bool {
        self.looping
    }

    /// Effective crop length, or zero while no positive range can be admitted.
    pub fn duration(&self) -> Duration {
        self.source_range.map_or(Duration::ZERO, SourceRange::duration)
    }
    pub(super) fn source_range(&self) -> Option<SourceRange> {
        self.source_range
    }

    /// Observe the immutable clock mapping. Reading never accumulates rounded frame periods.
    pub fn sample(&mut self) -> Result<TransportSample, TransportError> {
        let time = self.authority.sample()?;
        self.project(time.show_position().as_nanos(), time.mapping())
    }

    #[cfg(feature = "video-files")]
    pub(super) fn sample_at(&self, time: MonotonicTime) -> Result<TransportSample, TransportError> {
        let mapping = self.authority.mapping();
        self.project(mapping.show_at(time)?.as_nanos(), mapping)
    }

    fn project(&self, raw: i64, mapping: TimeMapping) -> Result<TransportSample, TransportError> {
        let reverse = self.rate.numerator() < 0;
        let crop_duration = self.duration();
        let duration = nanos(crop_duration)?;
        let timeline = (self.looping && !self.loop_overlap.is_zero())
            .then(|| LoopTimeline::new(crop_duration, self.loop_overlap))
            .flatten();
        let (position, cycle, playing) = if self.playing && self.looping && duration > 0 {
            if let Some(timeline) = timeline {
                let projected = timeline
                    .sample(ShowPosition::from_nanos(raw), reverse)
                    .ok_or(TransportError::Overflow)?;
                (nanos(projected.position)?, projected.cycle, true)
            } else {
                let position = raw.rem_euclid(duration);
                let cycle = raw.div_euclid(duration);
                if reverse && position == 0 {
                    (duration, cycle.checked_sub(1).ok_or(TransportError::Overflow)?, true)
                } else {
                    (position, cycle, true)
                }
            }
        } else {
            let ended = duration == 0
                || (self.rate.numerator() > 0 && raw >= duration)
                || (self.rate.numerator() < 0 && raw <= 0);
            (raw.clamp(0, duration), 0, self.playing && !ended)
        };
        Ok(TransportSample {
            epoch: mapping.epoch(),
            cycle,
            position: Duration::from_nanos(position as u64),
            playing,
            mapping,
            duration: crop_duration,
            looping: self.looping,
            source_range: self.source_range,
            timeline,
            reverse,
        })
    }

    /// Atomically adopt freshly probed duration without resetting healthy channel consumers.
    pub fn set_duration(&mut self, duration: Duration) -> Result<(), TransportError> {
        nanos(duration)?;
        self.set_source_range(SourceRange::new(Duration::ZERO, duration))
    }

    pub(super) fn set_source_range(&mut self, range: Option<SourceRange>) -> Result<(), TransportError> {
        if range == self.source_range {
            return Ok(());
        }
        let current = self.sample()?;
        let position = current.source_range.zip(range).map_or(Duration::ZERO, |(old, new)| {
            new.relative_position(old.source_position(current.position))
        });
        self.reanchor(nanos(position)?, current.playing && range.is_some())?;
        self.source_range = range;
        Ok(())
    }

    /// Resume in the selected direction; restarting a completed non-looping media starts over.
    pub fn play(&mut self) -> Result<(), TransportError> {
        let duration = nanos(self.duration())?;
        if duration == 0 {
            return Err(TransportError::NoDuration);
        }
        let current = self.sample()?;
        if current.playing {
            return Ok(());
        }
        let mut position = nanos(current.position)?;
        if self.rate.numerator() > 0 && position == duration {
            position = 0;
        } else if self.rate.numerator() < 0 && position == 0 {
            position = duration;
        }
        self.reanchor(position, true)
    }

    /// Resume the exact retained coordinate, including terminal boundaries. Matrix activation
    /// chooses this behavior explicitly instead of ordinary Play's endpoint rewind.
    pub(super) fn resume_position(&mut self) -> Result<(), TransportError> {
        if self.source_range.is_none() {
            return Err(TransportError::NoDuration);
        }
        let current = self.sample()?;
        if current.playing {
            return Ok(());
        }
        self.reanchor(nanos(current.position)?, true)
    }

    /// Freeze at the current shared position, without changing the chosen rate.
    pub fn pause(&mut self) -> Result<(), TransportError> {
        let current = self.sample()?;
        if !current.playing {
            return Ok(());
        }
        self.reanchor(nanos(current.position)?, false)
    }

    /// Pause and return every channel to the shared zero position.
    pub fn stop(&mut self) -> Result<(), TransportError> {
        self.reanchor(0, false)
    }

    /// Seek all consumers to one position while retaining the effective playback state.
    pub fn seek(&mut self, position: Duration) -> Result<(), TransportError> {
        let position = self.validate_position(position)?;
        let current = self.sample()?;
        self.reanchor(position, current.playing)
    }

    /// Pause at the selected channel's exact decoder-resolved timestamp.
    /// The app worker resolves the neighbour; UI callers must not estimate it from nominal FPS.
    pub fn step_to_timestamp(&mut self, timestamp: Duration) -> Result<(), TransportError> {
        let position = self.validate_position(timestamp)?;
        self.reanchor(position, false)
    }

    /// Change speed/direction without a jump; zero rate freezes the mapping explicitly.
    pub fn set_rate(&mut self, rate: PlaybackRate) -> Result<(), TransportError> {
        if rate == self.rate {
            return Ok(());
        }
        let current = self.sample()?;
        // A paused mapping stays paused; changing its future speed cannot invalidate the
        // exact frame neighbours needed by an immediately following Step command.
        if !current.playing {
            self.rate = rate;
            return Ok(());
        }
        self.authority
            .discontinuity(ShowPosition::from_nanos(nanos(current.position)?), rate)?;
        self.rate = rate;
        self.playing = current.playing;
        Ok(())
    }

    /// Change loop policy while preserving the current visible position.
    pub fn set_looping(&mut self, looping: bool) -> Result<(), TransportError> {
        self.set_loop_policy(looping, Duration::ZERO)
    }

    pub(super) fn set_loop_policy(&mut self, looping: bool, overlap: Duration) -> Result<(), TransportError> {
        let overlap = if looping { overlap } else { Duration::ZERO };
        if looping == self.looping && overlap == self.loop_overlap {
            return Ok(());
        }
        let current = self.sample()?;
        self.reanchor(nanos(current.position)?, current.playing)?;
        self.looping = looping;
        self.loop_overlap = overlap;
        Ok(())
    }

    fn validate_position(&self, position: Duration) -> Result<i64, TransportError> {
        let position = nanos(position)?;
        let duration = nanos(self.duration())?;
        if duration == 0 {
            return Err(TransportError::NoDuration);
        }
        if position > duration {
            return Err(TransportError::OutsideDuration);
        }
        Ok(position)
    }

    fn reanchor(&mut self, position: i64, playing: bool) -> Result<(), TransportError> {
        self.authority.discontinuity(
            ShowPosition::from_nanos(position),
            if playing { self.rate } else { PlaybackRate::PAUSED },
        )?;
        self.playing = playing;
        Ok(())
    }
}

fn nanos(value: Duration) -> Result<i64, TransportError> {
    i64::try_from(value.as_nanos()).map_err(|_| TransportError::Overflow)
}
