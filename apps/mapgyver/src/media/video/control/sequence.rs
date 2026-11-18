use super::State;
#[cfg(feature = "video-files")]
use super::VideoControl;
#[cfg(feature = "video-files")]
use crate::media::{
    protocol::{AssetLocator, ChannelId},
    video::TransportSample,
};
use std::time::Duration;

impl State {
    pub(super) fn update_repeat(&mut self) -> Result<(), String> {
        let separate = self
            .files
            .values()
            .any(|file| file.locator.is_some() && file.repeat.is_some());
        let duration = self.repeat_durations().max().unwrap_or_default();
        let shortest = self.repeat_durations().min();
        if (
            self.separate_repeat,
            self.repeat_duration,
            self.shortest_repeat_duration,
        ) == (separate, duration, shortest)
        {
            return Ok(());
        }
        let extended = duration > self.repeat_duration;
        self.separate_repeat = separate;
        self.repeat_duration = duration;
        self.shortest_repeat_duration = shortest;
        let resume = extended
            && self.settings.looping
            && self.transport.sample().map_err(|err| err.to_string())?.has_finite_end();
        self.apply(false)?;
        if resume && self.settings.mode == super::VideoPlaybackMode::Playing && !self.transport.duration().is_zero() {
            self.transport.resume_position().map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    fn repeat_durations(&self) -> impl Iterator<Item = Duration> {
        self.files.values().filter_map(|file| {
            file.locator.as_ref()?;
            file.repeat?.duration
        })
    }

    #[cfg(feature = "video-files")]
    pub(super) fn repeat_boundary(&self) -> Option<Duration> {
        (self.settings.looping && self.separate_repeat).then(|| self.intro_bounds().1)
    }
}

#[cfg(feature = "video-files")]
impl VideoControl {
    pub(crate) fn sequence_file(&self, channel: ChannelId) -> Result<Option<Option<AssetLocator>>, String> {
        let state = self.0.lock().map_err(|_| "Video control poisoned")?;
        // Siblings without an alternate file still share the extended timeline, holding their
        // final intro image while channels with a loop file repeat.
        Ok(state.separate_repeat.then(|| {
            state
                .files
                .get(&channel)
                .and_then(|file| file.loop_files.picture.clone())
        }))
    }

    pub(crate) fn sequence_sample(&self) -> Result<(TransportSample, Option<Duration>), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        Ok((
            state.transport.sample().map_err(|err| err.to_string())?,
            state.repeat_boundary(),
        ))
    }
}
