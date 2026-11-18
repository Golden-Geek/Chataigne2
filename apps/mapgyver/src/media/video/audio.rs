//! Audio observes the same crop, loop and authority epoch as video frames.
use crate::{
    audio::voice::Binding,
    media::protocol::{AssetLocator, ChannelId},
};
use golden_time::{ShowPosition, TimeEpoch};
use std::{sync::Arc, time::Duration};

use super::TransportSample;

pub(crate) struct Source {
    pub channel: ChannelId,
    pub generation: uuid::Uuid,
    pub file: Option<AssetLocator>,
    pub duration: Option<Duration>,
    pub binding: Arc<Binding>,
    pub repeat: Option<(AssetLocator, Option<Duration>)>,
}

#[derive(Clone, Copy)]
pub(crate) struct AudioTimeline {
    sample: TransportSample,
    separate_boundary: Option<Duration>,
}

#[derive(Clone, Copy)]
pub(crate) struct AudioSample {
    pub position: Duration,
    pub weight: f32,
    pub repeat: bool,
}

impl AudioTimeline {
    #[cfg(test)]
    pub(super) fn new(sample: TransportSample) -> Option<Self> {
        Self::with_repeat(sample, None)
    }

    pub(super) fn with_repeat(sample: TransportSample, separate_boundary: Option<Duration>) -> Option<Self> {
        (sample.playing && sample.mapping.rate().numerator() != 0 && !sample.duration.is_zero()).then_some(Self {
            sample,
            separate_boundary,
        })
    }

    pub fn epoch(self) -> TimeEpoch {
        self.sample.epoch
    }

    #[cfg(test)]
    pub fn positions(self, offset_ns: u64, channel_duration: Duration) -> [Option<(Duration, f32)>; 2] {
        self.samples(offset_ns, channel_duration, None)
            .map(|sample| sample.map(|sample| (sample.position, sample.weight)))
    }

    pub fn samples(
        self,
        offset_ns: u64,
        channel_duration: Duration,
        alternate_duration: Option<Duration>,
    ) -> [Option<AudioSample>; 2] {
        let sample = self.sample;
        let rate = sample.mapping.rate();
        let step = i128::from(offset_ns) * i128::from(rate.numerator()) / i128::from(rate.denominator());
        let raw = sample.unwrapped_position() + step;
        let crop_end = sample.duration.as_nanos() as i128;
        let reverse = rate.numerator() < 0;
        let (position, blend) = if let Some(timeline) = sample.timeline {
            let Some(projected) = i64::try_from(raw)
                .ok()
                .and_then(|raw| timeline.sample(ShowPosition::from_nanos(raw), reverse))
            else {
                return [None, None];
            };
            (projected.position, projected.blend)
        } else {
            let position = if sample.looping {
                if reverse {
                    (raw - 1).rem_euclid(crop_end) + 1
                } else {
                    raw.rem_euclid(crop_end)
                }
            } else if raw < 0 || raw >= crop_end && !reverse || raw > crop_end {
                return [None, None];
            } else {
                raw
            };
            (Duration::from_nanos(position as u64), None)
        };
        let alternate_boundary = self.separate_boundary.or_else(|| {
            let range = sample.source_range?;
            (sample.looping && alternate_duration.is_some())
                .then(|| range.start() + sample.timeline.map_or(Duration::ZERO, |timeline| timeline.loop_start()))
        });
        let sample_source = |position: Duration, weight| {
            let position = if reverse {
                position.saturating_sub(Duration::from_nanos(1))
            } else {
                position
            };
            let source = sample.source_position(position, Duration::MAX);
            let (position, limit, repeat) = if let Some(boundary) = alternate_boundary.filter(|end| source >= *end) {
                (source - boundary, alternate_duration?, true)
            } else {
                (source, channel_duration, false)
            };
            (position < limit && weight > 0.0).then_some(AudioSample {
                position,
                weight,
                repeat,
            })
        };
        if let Some(blend) = blend {
            [
                sample_source(position, blend.primary_weight()),
                sample_source(blend.secondary_position, blend.secondary_weight),
            ]
        } else {
            [sample_source(position, 1.0), None]
        }
    }
}
