#[cfg(all(windows, feature = "native-video", feature = "video-files"))]
use golden_time::MonotonicTime;
use golden_time::{Clock, PlaybackRate, ShowPosition, TimeAuthority, TimeEpoch, TimeError, TimeMapping};
use std::time::Duration;

/// One immutable media-wide observation. Every channel samples this same position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportSample {
    /// Authority identity and explicit-control discontinuity fence.
    pub epoch: TimeEpoch,
    /// Loop cycle within the epoch; a seek result from an old cycle must not publish.
    pub cycle: i64,
    /// Shared position, clipped or wrapped against the longest channel's duration.
    pub position: Duration,
    /// Effective playback state, false at a non-looping endpoint.
    pub playing: bool,
    pub(crate) mapping: TimeMapping,
    pub(crate) duration: Duration,
    pub(crate) looping: bool,
}

impl TransportSample {
    pub(crate) fn frame_position(self) -> Result<golden_video::scheduling::FramePosition, String> {
        let raw = i128::from(self.cycle) * self.duration.as_nanos() as i128 + self.position.as_nanos() as i128;
        // Reverse looping maps the exact seam to the previous cycle's final image.
        let raw =
            raw - i128::from(self.looping && self.mapping.rate().numerator() < 0 && self.position == self.duration);
        Ok(golden_video::scheduling::FramePosition {
            epoch: self.epoch,
            position: ShowPosition::from_nanos(i64::try_from(raw).map_err(|_| "Video presentation overflow")?),
            reverse: self.mapping.rate().numerator() < 0,
        })
    }
    /// Shorter channels hold their last available frame until the whole media reaches its end.
    /// The decoder selects the frame at/before this timestamp; duration is not a frame number.
    pub fn channel_position(self, channel_duration: Duration) -> Duration {
        self.position.min(channel_duration)
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
    duration: i64,
    rate: PlaybackRate,
    playing: bool,
    looping: bool,
}

impl<C: Clock> MediaTransport<C> {
    /// New Video starts paused at zero, rate one and looping off.
    pub fn new(clock: C) -> Result<Self, TransportError> {
        Ok(Self {
            authority: TimeAuthority::new(clock, ShowPosition::from_nanos(0), PlaybackRate::PAUSED)?,
            duration: 0,
            rate: PlaybackRate::NORMAL,
            playing: false,
            looping: false,
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

    /// Duration admitted by the file worker: the longest ready channel.
    pub fn duration(&self) -> Duration {
        Duration::from_nanos(self.duration as u64)
    }

    /// Observe the immutable clock mapping. Reading never accumulates rounded frame periods.
    pub fn sample(&mut self) -> Result<TransportSample, TransportError> {
        let time = self.authority.sample()?;
        self.project(time.show_position().as_nanos(), time.mapping())
    }

    #[cfg(all(windows, feature = "native-video", feature = "video-files"))]
    pub(super) fn sample_at(&self, time: MonotonicTime) -> Result<TransportSample, TransportError> {
        let mapping = self.authority.mapping();
        self.project(mapping.show_at(time)?.as_nanos(), mapping)
    }

    fn project(&self, raw: i64, mapping: TimeMapping) -> Result<TransportSample, TransportError> {
        let (position, cycle, playing) = if self.playing && self.looping && self.duration > 0 {
            let position = raw.rem_euclid(self.duration);
            let cycle = raw.div_euclid(self.duration);
            if self.rate.numerator() < 0 && position == 0 {
                (
                    self.duration,
                    cycle.checked_sub(1).ok_or(TransportError::Overflow)?,
                    true,
                )
            } else {
                (position, cycle, true)
            }
        } else {
            let ended = self.duration == 0
                || (self.rate.numerator() > 0 && raw >= self.duration)
                || (self.rate.numerator() < 0 && raw <= 0);
            (raw.clamp(0, self.duration), 0, self.playing && !ended)
        };
        Ok(TransportSample {
            epoch: mapping.epoch(),
            cycle,
            position: Duration::from_nanos(position as u64),
            playing,
            mapping,
            duration: self.duration(),
            looping: self.looping,
        })
    }

    /// Atomically adopt freshly probed duration without resetting healthy channel consumers.
    pub fn set_duration(&mut self, duration: Duration) -> Result<(), TransportError> {
        let duration = nanos(duration)?;
        if duration == self.duration {
            return Ok(());
        }
        let current = self.sample()?;
        self.reanchor(
            current.position.as_nanos().min(duration as u128) as i64,
            current.playing,
        )?;
        self.duration = duration;
        Ok(())
    }

    /// Resume in the selected direction; restarting a completed non-looping media starts over.
    pub fn play(&mut self) -> Result<(), TransportError> {
        if self.duration == 0 {
            return Err(TransportError::NoDuration);
        }
        let current = self.sample()?;
        if current.playing {
            return Ok(());
        }
        let mut position = nanos(current.position)?;
        if self.rate.numerator() > 0 && position == self.duration {
            position = 0;
        } else if self.rate.numerator() < 0 && position == 0 {
            position = self.duration;
        }
        self.reanchor(position, true)
    }

    /// Resume the exact retained coordinate, including terminal boundaries. Matrix activation
    /// chooses this behavior explicitly instead of ordinary Play's endpoint rewind.
    pub(super) fn resume_position(&mut self) -> Result<(), TransportError> {
        if self.duration == 0 {
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
        if looping == self.looping {
            return Ok(());
        }
        let current = self.sample()?;
        self.reanchor(nanos(current.position)?, current.playing)?;
        self.looping = looping;
        Ok(())
    }

    fn validate_position(&self, position: Duration) -> Result<i64, TransportError> {
        let position = nanos(position)?;
        if self.duration == 0 {
            return Err(TransportError::NoDuration);
        }
        if position > self.duration {
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
