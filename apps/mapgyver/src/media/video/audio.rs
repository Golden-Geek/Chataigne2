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
}

#[derive(Clone, Copy)]
pub(crate) struct AudioTimeline(TransportSample);

impl AudioTimeline {
    pub(super) fn new(sample: TransportSample) -> Option<Self> {
        (sample.playing && sample.mapping.rate().numerator() != 0 && !sample.duration.is_zero()).then_some(Self(sample))
    }

    pub fn epoch(self) -> TimeEpoch {
        self.0.epoch
    }

    pub fn positions(self, offset_ns: u64, channel_duration: Duration) -> [Option<(Duration, f32)>; 2] {
        let Self(sample) = self;
        let rate = sample.mapping.rate();
        let step = i128::from(offset_ns) * i128::from(rate.numerator()) / i128::from(rate.denominator());
        let raw = sample.cycle_origin() + sample.position.as_nanos() as i128 + step;
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
        let sample_source = |position: Duration, weight| {
            let position = if reverse {
                position.saturating_sub(Duration::from_nanos(1))
            } else {
                position
            };
            let source = sample.source_position(position, channel_duration);
            (source < channel_duration && weight > 0.0).then_some((source, weight))
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
