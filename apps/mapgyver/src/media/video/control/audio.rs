use super::VideoControl;
use crate::media::{
    protocol::ChannelId,
    video::audio::{AudioTimeline, Source},
};
use std::time::Instant;

impl VideoControl {
    pub(crate) fn audio_sources(&self) -> Vec<Source> {
        let Ok(state) = self.0.lock() else {
            return Vec::new();
        };
        if state.retired {
            return Vec::new();
        }
        state
            .files
            .iter()
            .map(|(channel, file)| Source {
                channel: *channel,
                generation: file.generation,
                file: file.locator.clone(),
                duration: file.duration,
                binding: file.audio.clone(),
            })
            .collect()
    }

    pub(crate) fn audio_timeline_at(&self, instant: Instant) -> Option<AudioTimeline> {
        let state = self.0.lock().ok()?;
        if state.retired {
            return None;
        }
        let sample = state
            .transport
            .sample_at(state.clock.monotonic_at(instant).ok()?)
            .ok()?;
        AudioTimeline::new(sample)
    }

    pub(crate) fn owns_audio(&self, channel: ChannelId, generation: uuid::Uuid) -> bool {
        self.0.lock().is_ok_and(|state| {
            !state.retired
                && state
                    .files
                    .get(&channel)
                    .is_some_and(|file| file.generation == generation && file.metadata_error.is_none())
        })
    }
}
