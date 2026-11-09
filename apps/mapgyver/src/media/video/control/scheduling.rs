//! Media policy projects decoded PTS windows onto Golden's bounded presentation mechanism.
use crate::media::protocol::ChannelId;
use crate::media::video::{TransportSample, VideoControl, frames::Neighbours};
use golden_time::ShowPosition;
use golden_video::{
    native::{GpuFrameClock, GpuFramePublication, GpuFrameStream, GpuImage},
    scheduling::{FramePosition, FrameWindow},
};
use std::time::{Duration, Instant};

impl GpuFrameClock for VideoControl {
    fn position_at(&self, instant: Instant) -> Option<FramePosition> {
        let state = self.0.try_lock().ok()?;
        state
            .transport
            .sample_at(state.clock.monotonic_at(instant).ok()?)
            .ok()?
            .frame_position()
            .ok()
    }
}

impl VideoControl {
    pub(in crate::media::video) fn publish_gpu_scheduled(
        &self,
        channel: ChannelId,
        owner: uuid::Uuid,
        expected: TransportSample,
        stream: &GpuFrameStream,
        image: GpuImage,
        frame: Neighbours,
    ) -> Result<bool, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        if !state.frames.owns(channel, owner) || now.epoch != expected.epoch {
            return Ok(false);
        }
        if !timed(now) {
            let retired = stream.replace(Some(image)).map_err(str::to_owned)?;
            state.frames.observe(channel, owner, now, frame);
            state.frames.ready(channel, owner);
            drop(state);
            drop(retired);
            return Ok(true);
        }
        let window = window(expected, frame)?;
        let position = now.frame_position()?;
        let result = stream
            .replace_scheduled(image, window, position)
            .map_err(str::to_owned)?;
        let accepted = matches!(result, GpuFramePublication::Published(_));
        let metadata = if accepted {
            let result = state.frames.observe_scheduled(channel, owner, window, frame, position);
            state.frames.ready(channel, owner);
            result
        } else {
            Ok(())
        };
        drop(state);
        drop(result);
        metadata?;
        Ok(accepted)
    }
    pub(in crate::media::video) fn refresh_gpu_neighbours(
        &self,
        channel: ChannelId,
        owner: uuid::Uuid,
        expected: TransportSample,
        frame: Neighbours,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        if state.frames.owns(channel, owner) && now.epoch == expected.epoch {
            if !timed(now) {
                state.frames.observe(channel, owner, now, frame);
                return Ok(());
            }
            state
                .frames
                .observe_scheduled(channel, owner, window(expected, frame)?, frame, now.frame_position()?)?;
        }
        Ok(())
    }
}

pub(in crate::media::video) fn timed(sample: TransportSample) -> bool {
    sample.playing && sample.mapping.rate().numerator() != 0
}

pub(in crate::media::video) fn window(sample: TransportSample, frame: Neighbours) -> Result<FrameWindow, String> {
    let origin = i128::from(sample.cycle) * sample.duration.as_nanos() as i128;
    let coordinate = |position: Duration| -> Result<ShowPosition, String> {
        i64::try_from(origin + position.as_nanos() as i128)
            .map(ShowPosition::from_nanos)
            .map_err(|_| "Video frame window overflow".into())
    };
    let end = frame
        .next
        .or(sample.looping.then_some(sample.duration))
        .map(coordinate)
        .transpose()?;
    FrameWindow::new(
        sample.epoch,
        coordinate(frame.position)?,
        end,
        ShowPosition::from_nanos(i64::try_from(frame.position.as_nanos()).map_err(|_| "Video PTS overflow")?),
    )
    .ok_or_else(|| "Invalid Video frame interval".into())
}

/// Only one forward future frame is prepared. Reverse retains its exact bounded seek route.
pub(in crate::media::video) fn target(
    sample: TransportSample,
    cursor: Option<TransportSample>,
    frame: Option<Neighbours>,
    published: bool,
    channel_duration: Duration,
) -> Result<Option<TransportSample>, String> {
    let Some(cursor) = cursor.filter(|cursor| cursor.epoch == sample.epoch) else {
        return Ok(Some(sample));
    };
    if !sample.playing || sample.mapping.rate().numerator() <= 0 {
        return Ok(Some(sample));
    }
    if cursor.cycle > sample.cycle {
        return Ok(None);
    }
    if cursor.cycle < sample.cycle {
        return Ok(Some(sample));
    }
    let Some(frame) = frame else {
        return Ok(Some(sample));
    };
    let current = sample.channel_position(channel_duration);
    if frame.position > current {
        return Ok(None);
    }
    if !published || frame.next.is_some_and(|next| next <= current) {
        return Ok(Some(sample));
    }
    if let Some(next) = frame.next {
        return Ok(Some(TransportSample {
            position: next,
            ..sample
        }));
    }
    if sample.looping {
        return Ok(Some(TransportSample {
            cycle: sample.cycle.checked_add(1).ok_or("Video loop overflow")?,
            position: Duration::ZERO,
            ..sample
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
