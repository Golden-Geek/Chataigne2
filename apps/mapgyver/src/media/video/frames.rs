//! Bounded source-owned timestamp observations; no decoding on parameter/operation callers.
use super::{TransportSample, protocol::*};
use crate::media::protocol::{AssetLocator, ChannelId, ChannelReference, MediaId};
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Neighbours {
    pub position: Duration,
    pub previous: Option<Duration>,
    pub previous_ready: bool,
    pub next: Option<Duration>,
}
struct Source {
    owner: uuid::Uuid,
    file: AssetLocator,
    frame: Option<(TransportSample, Neighbours)>,
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
    #[cfg(any(feature = "video-files", test))]
    pub fn observe(&mut self, channel: ChannelId, owner: uuid::Uuid, sample: TransportSample, frame: Neighbours) {
        if let Some(source) = self.0.get_mut(&channel)
            && source.owner == owner
        {
            source.frame = Some((sample, frame));
        }
    }
    fn current(&self, channel: ChannelId, file: &AssetLocator, now: TransportSample) -> Option<Neighbours> {
        let source = self.0.get(&channel)?;
        let (sample, frame) = source.frame?;
        (source.file == *file && (sample.epoch, sample.cycle) == (now.epoch, now.cycle)).then_some(frame)
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
