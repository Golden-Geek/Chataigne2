//! Bounded source-owned timestamp observations; no decoding on parameter/operation callers.
use super::{TransportSample, protocol::*};
use crate::media::protocol::{AssetLocator, ChannelId, ChannelReference, MediaId};
use golden_video::scheduling::SourceRange;
use std::{collections::HashMap, time::Duration};

pub(super) fn window(
    sample: TransportSample,
    frame: Neighbours,
) -> Result<golden_video::scheduling::FrameWindow, String> {
    let range = sample.source_range.ok_or("Video frame range unavailable")?;
    let timed_loop = sample.looping && sample.playing;
    let active = if timed_loop {
        sample.cycle_start()..sample.cycle_end()
    } else {
        Duration::ZERO..range.duration()
    };
    range
        .frame_window(
            sample.epoch,
            sample.cycle_origin(),
            active,
            frame.position,
            frame.next_in_range(range),
            !timed_loop,
        )
        .ok_or_else(|| "Invalid Video frame interval".into())
}

/// Split a decoded interval at the start of the seam and into bounded fade samples.
/// The same clock window gates both the published image and frame-step observations.
pub(super) fn loop_window(
    sample: TransportSample,
    frame: Neighbours,
) -> Result<golden_video::scheduling::FrameWindow, String> {
    use golden_time::ShowPosition;
    use golden_video::scheduling::FrameWindow;
    let window = window(sample, frame)?;
    let Some(timeline) = sample.timeline.filter(|_| sample.playing) else {
        return Ok(window);
    };
    let position = sample.frame_position()?.position.as_nanos();
    let origin = sample.cycle_origin();
    let seam_start = origin
        + if sample.reverse {
            0
        } else {
            timeline.cycle_duration().as_nanos() as i128
        };
    let seam_end = seam_start + timeline.overlap().as_nanos() as i128;
    let position = i128::from(position);
    let (lower, upper) = if position < seam_start {
        (i128::MIN, seam_start)
    } else if position >= seam_end {
        (seam_end, i128::MAX)
    } else {
        // Use an absolute media-time grid to avoid an accumulation of rounded periods.
        const QUANTUM_NS: i128 = 16_666_667;
        let lower = seam_start + (position - seam_start) / QUANTUM_NS * QUANTUM_NS;
        (lower, (lower + QUANTUM_NS).min(seam_end))
    };
    let start = window
        .start()
        .as_nanos()
        .max(i64::try_from(lower.max(i128::from(i64::MIN))).map_err(|_| "Video loop start overflows")?);
    let end = window
        .end()
        .map(|end| end.as_nanos())
        .unwrap_or(i64::MAX)
        .min(i64::try_from(upper.min(i128::from(i64::MAX))).map_err(|_| "Video loop end overflows")?);
    FrameWindow::new(
        sample.epoch,
        ShowPosition::from_nanos(start),
        Some(ShowPosition::from_nanos(end)),
        window.pts(),
    )
    .ok_or_else(|| "Invalid Video loop frame interval".into())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Neighbours {
    /// Timestamp in the original source, before cropping or looping.
    pub position: Duration,
    pub previous: Option<Duration>,
    pub previous_ready: bool,
    pub next: Option<Duration>,
}
impl Neighbours {
    pub(super) fn next_in_range(self, range: SourceRange) -> Option<Duration> {
        // A channel shorter than the crop's Start holds its last image.
        self.next.filter(|next| range.contains(*next) && *next > range.start())
    }

    fn relative(self, range: SourceRange) -> Self {
        let position = range.relative_position(self.position);
        Self {
            position,
            previous: self
                .previous
                .map(|previous| range.relative_position(previous))
                .filter(|previous| *previous < position),
            previous_ready: self.previous_ready || position.is_zero(),
            next: self.next_in_range(range).map(|next| range.relative_position(next)),
        }
    }
}
struct Source {
    owner: uuid::Uuid,
    file: AssetLocator,
    frame: Option<(TransportSample, Neighbours)>,
    scheduled: golden_video::scheduling::FrameBuffer<Neighbours>,
    ready: bool,
    error: Option<VideoSourceError>,
}
#[derive(Default)]
pub(super) struct Frames(HashMap<ChannelId, Source>);
impl Frames {
    #[cfg(feature = "video-files")]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn claim(&mut self, channel: ChannelId, owner: uuid::Uuid, file: AssetLocator) {
        self.0.insert(
            channel,
            Source {
                owner,
                file,
                frame: None,
                scheduled: Default::default(),
                ready: false,
                error: None,
            },
        );
    }
    pub fn owns(&self, channel: ChannelId, owner: uuid::Uuid) -> bool {
        self.0.get(&channel).is_some_and(|source| source.owner == owner)
    }
    pub fn file(&self, channel: ChannelId, owner: uuid::Uuid) -> Option<&AssetLocator> {
        self.0
            .get(&channel)
            .filter(|source| source.owner == owner)
            .map(|source| &source.file)
    }
    pub fn remove(&mut self, channel: ChannelId) {
        self.0.remove(&channel);
    }
    pub fn release(&mut self, channel: ChannelId, owner: uuid::Uuid) {
        if self.owns(channel, owner) {
            self.0.remove(&channel);
        }
    }
    pub fn status(&self, channel: ChannelId, file: &AssetLocator) -> Option<(bool, Option<VideoSourceError>)> {
        self.0
            .get(&channel)
            .filter(|source| source.file == *file)
            .map(|source| (source.ready, source.error.clone()))
    }
    pub fn ready(&mut self, channel: ChannelId, owner: uuid::Uuid) {
        if let Some(source) = self.0.get_mut(&channel).filter(|source| source.owner == owner) {
            source.ready = true;
            source.error = None;
        }
    }
    #[cfg(feature = "video-files")]
    pub fn failure(&mut self, channel: ChannelId, owner: uuid::Uuid, error: VideoSourceError) -> bool {
        let Some(source) = self.0.get_mut(&channel).filter(|source| source.owner == owner) else {
            return false;
        };
        source.frame = None;
        source.scheduled.clear();
        if source.error.as_ref() == Some(&error) {
            return false;
        }
        source.error = Some(error);
        true
    }
    #[cfg(any(feature = "video-files", test))]
    pub fn observe(&mut self, channel: ChannelId, owner: uuid::Uuid, sample: TransportSample, frame: Neighbours) {
        if let Some(source) = self.0.get_mut(&channel)
            && source.owner == owner
        {
            source.frame = Some((sample, frame));
            source.scheduled.clear();
        }
    }
    fn current(&self, channel: ChannelId, file: &AssetLocator, now: TransportSample) -> Option<Neighbours> {
        let range = now.source_range?;
        let source = self.0.get(&channel)?;
        if source.file != *file {
            return None;
        }
        if !source.scheduled.is_empty() {
            return source
                .scheduled
                .sample(now.frame_position().ok()?)
                .map(|(_, frame)| frame.relative(range));
        }
        let (sample, frame) = source.frame?;
        ((sample.epoch, sample.cycle) == (now.epoch, now.cycle)).then(|| frame.relative(range))
    }
    #[cfg(all(windows, feature = "native-video", feature = "video-files"))]
    pub(super) fn observe_scheduled(
        &mut self,
        channel: ChannelId,
        owner: uuid::Uuid,
        window: golden_video::scheduling::FrameWindow,
        frame: Neighbours,
        now: golden_video::scheduling::FramePosition,
    ) -> Result<(), String> {
        let source = self
            .0
            .get_mut(&channel)
            .filter(|source| source.owner == owner)
            .ok_or("Video frame owner changed")?;
        source
            .scheduled
            .publish(window, frame, now)
            .map_err(|_| "Video frame metadata capacity diverged")?;
        Ok(())
    }
    pub fn inspect(
        &self,
        media: MediaId,
        channel: ChannelId,
        file: &AssetLocator,
        now: TransportSample,
    ) -> Option<VideoFrameState> {
        let frame = self.current(channel, file, now)?;
        Some(VideoFrameState {
            reference: ChannelReference { media, channel },
            position_ms: millis(frame.position),
            previous_ms: frame.previous.map(millis),
            previous_ready: frame.previous_ready,
            next_ms: frame.next.map(millis),
        })
    }
    pub fn step(
        &self,
        channel: ChannelId,
        file: &AssetLocator,
        now: TransportSample,
        direction: VideoStepDirection,
    ) -> Result<Duration, String> {
        let frame = self
            .current(channel, file, now)
            .ok_or("Selected Video channel timestamps are not ready")?;
        match direction {
            VideoStepDirection::Previous if !frame.previous_ready => {
                Err("Previous Video frame timestamp is not ready".into())
            }
            VideoStepDirection::Previous => Ok(frame.previous.unwrap_or(now.position)),
            VideoStepDirection::Next => Ok(frame.next.unwrap_or(now.position)),
        }
    }
}
pub(crate) fn millis(position: Duration) -> f64 {
    let mut value = position.as_secs_f64() * 1000.0;
    // A rounded-down millisecond value can select the preceding frame in a rational time base.
    while Duration::from_secs_f64(value / 1000.0) < position {
        value = value.next_up();
    }
    value
}
