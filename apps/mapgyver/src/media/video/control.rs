use super::{MediaTransport, protocol::*};
use golden_time::{PlaybackRate, SystemClock};
use std::{sync::Mutex, time::Duration};

struct State {
    transport: MediaTransport<SystemClock>,
    settings: VideoTransportSettings,
    seek_pending: bool,
    seek_waiting_for_duration: bool,
    retired: bool,
    files: std::collections::HashMap<super::super::protocol::ChannelId, super::bindings::File>,
    durations: std::collections::BTreeMap<Duration, usize>,
    frames: super::frames::Frames,
}

/// Shared per-media runtime control, retained by demanded sources rather than per Reference.
pub struct VideoControl(Mutex<State>, golden_io::RevisionSource);
impl Default for VideoControl {
    fn default() -> Self {
        Self(
            Mutex::new(State {
                transport: MediaTransport::new(SystemClock::new()).expect("fresh monotonic clock is representable"),
                settings: Default::default(),
                seek_pending: true,
                seek_waiting_for_duration: false,
                retired: false,
                files: Default::default(),
                durations: Default::default(),
                frames: Default::default(),
            }),
            Default::default(),
        )
    }
}
impl VideoControl {
    pub(crate) fn source_status(
        &self,
        reference: super::super::protocol::ChannelReference,
        locator: &Option<super::super::protocol::AssetLocator>,
    ) -> Result<VideoSourceStatus, String> {
        #[cfg(not(feature = "video-files"))]
        if locator.is_some() {
            return Ok(VideoSourceStatus {
                reference,
                state: VideoSourceState::Unavailable,
                duration_ms: None,
                retrying: false,
                error: Some(VideoSourceError::new(
                    VideoSourceErrorKind::Unsupported,
                    "This build does not support Video playback",
                )),
            });
        }
        let state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let binding = state.files.get(&reference.channel);
        if binding.is_some_and(|binding| &binding.locator != locator) {
            return Err("Video File binding changed before inspection".into());
        }
        let duration_ms = binding.and_then(|binding| binding.duration).map(super::frames::millis);
        let metadata_error = binding.and_then(|binding| binding.metadata_error.clone());
        let (phase, error) = if let Some(locator) = locator {
            match state.frames.status(reference.channel, locator) {
                Some((ready, Some(error))) => (
                    if ready {
                        VideoSourceState::Recovering
                    } else {
                        VideoSourceState::Unavailable
                    },
                    Some(error),
                ),
                Some((true, None)) => (VideoSourceState::Ready, None),
                _ if metadata_error.is_some() => (VideoSourceState::Unavailable, metadata_error),
                Some((false, None)) => (VideoSourceState::Loading, None),
                None if duration_ms.is_some() => (VideoSourceState::Prepared, None),
                None => (VideoSourceState::Loading, None),
            }
        } else {
            (VideoSourceState::Unconfigured, None)
        };
        Ok(VideoSourceStatus {
            reference,
            state: phase,
            duration_ms,
            retrying: error.is_some(),
            error,
        })
    }
    #[cfg(feature = "video-files")]
    pub(super) fn metadata_failure(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
        error: VideoSourceError,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let Some(file) = state
            .files
            .get_mut(&channel)
            .filter(|file| file.generation == generation)
        else {
            return Ok(());
        };
        if file.metadata_error.as_ref() == Some(&error) {
            return Ok(());
        }
        file.metadata_error = Some(error);
        drop(state);
        self.1.advance();
        Ok(())
    }
    #[cfg(feature = "video-files")]
    pub(super) fn source_failure(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        error: VideoSourceError,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let changed = state.frames.failure(channel, owner, error);
        drop(state);
        if changed {
            self.1.advance();
        }
        Ok(())
    }
    pub(crate) fn retire_media(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.retired = true;
        }
    }
    pub(crate) fn activate_media(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.retired = false;
        }
    }
    #[cfg(feature = "video-files")]
    pub(super) fn needs_metadata(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|state| !state.retired || !state.frames.is_empty())
    }
    pub(crate) fn bind_file(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        locator: Option<super::super::protocol::AssetLocator>,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state
            .files
            .get(&channel)
            .is_some_and(|file| file.owner == owner && file.locator == locator)
        {
            return Ok(());
        }
        if !state.files.contains_key(&channel) && state.files.len() >= 4096 {
            return Err("Video metadata exceeds 4096 authored channels".into());
        }
        state.frames.remove(channel);
        let old = state
            .files
            .insert(
                channel,
                super::bindings::File {
                    owner,
                    #[cfg(any(feature = "video-files", test))]
                    generation: uuid::Uuid::new_v4(),
                    locator,
                    duration: None,
                    metadata_error: None,
                },
            )
            .and_then(|file| file.duration);
        state.duration_changed(old, None)?;
        drop(state);
        self.1.advance();
        Ok(())
    }
    pub(crate) fn unbind_file(&self, channel: super::super::protocol::ChannelId, owner: uuid::Uuid) {
        if let Ok(mut state) = self.0.lock()
            && !state.retired
            && state.files.get(&channel).is_some_and(|file| file.owner == owner)
        {
            let old = state.files.remove(&channel).and_then(|file| file.duration);
            state.frames.remove(channel);
            let _ = state.duration_changed(old, None);
            drop(state);
            self.1.advance();
        }
    }
    #[cfg(any(feature = "video-files", test))]
    pub(super) fn metadata_files(
        &self,
    ) -> Result<
        Vec<(
            super::super::protocol::ChannelId,
            uuid::Uuid,
            super::super::protocol::AssetLocator,
        )>,
        String,
    > {
        let state = self.0.lock().map_err(|_| "Video control poisoned")?;
        Ok(state
            .files
            .iter()
            .filter_map(|(channel, file)| file.locator.clone().map(|locator| (*channel, file.generation, locator)))
            .collect())
    }
    #[cfg(any(feature = "video-files", test))]
    pub(super) fn metadata_duration(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
        duration: Duration,
    ) -> Result<bool, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let Some(file) = state
            .files
            .get_mut(&channel)
            .filter(|file| file.generation == generation)
        else {
            return Ok(false);
        };
        let recovered = file.metadata_error.take().is_some();
        if file.duration == Some(duration) {
            if recovered {
                drop(state);
                self.1.advance();
            }
            return Ok(true);
        }
        let old = file.duration.replace(duration);
        state.duration_changed(old, Some(duration))?;
        drop(state);
        self.1.advance();
        Ok(true)
    }
    #[cfg(feature = "video-files")]
    pub(super) fn metadata_is_current(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
    ) -> bool {
        self.0.lock().is_ok_and(|state| {
            state
                .files
                .get(&channel)
                .is_some_and(|file| file.generation == generation)
        })
    }
    pub fn claim_channel(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        file: super::super::protocol::AssetLocator,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.files.get(&channel).and_then(|binding| binding.locator.as_ref()) != Some(&file) {
            return Err("Video source no longer matches its authored File".into());
        }
        state.frames.claim(channel, owner, file);
        Ok(())
    }
    pub fn revisions(&self) -> golden_io::RevisionReader {
        self.1.reader()
    }
    pub fn sample(&self) -> Result<super::TransportSample, String> {
        self.0
            .lock()
            .map_err(|_| "Video control poisoned")?
            .transport
            .sample()
            .map_err(|e| e.to_string())
    }
    pub fn channel_duration(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        duration: Duration,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let file = state
            .frames
            .file(channel, owner)
            .ok_or("Video source owner was superseded")?
            .clone();
        let binding = state.files.get_mut(&channel).ok_or("Video channel was removed")?;
        if binding.locator.as_ref() != Some(&file) {
            return Err("Video File was relinked".into());
        }
        if binding.duration == Some(duration) {
            return Ok(());
        }
        let old = binding.duration.replace(duration);
        state.duration_changed(old, Some(duration))?;
        drop(state);
        self.1.advance();
        Ok(())
    }
    pub fn release_channel(&self, channel: super::super::protocol::ChannelId, owner: uuid::Uuid) {
        if let Ok(mut state) = self.0.lock() {
            // Demand belongs to a consumer, not to media transport. Keep known metadata
            // while previews/References are closed; resubscription must not reset time.
            state.frames.release(channel, owner);
        }
    }
    pub fn publish_frame(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        expected: super::TransportSample,
        stream: &golden_video::image::ImageStream,
        image: golden_video::image::ImageData,
    ) -> Result<bool, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        if !state.frames.owns(channel, owner) || (now.epoch, now.cycle) != (expected.epoch, expected.cycle) {
            return Ok(false);
        }
        let retired = stream.replace(Some(image)).map_err(str::to_owned)?;
        state.frames.ready(channel, owner);
        drop(state);
        drop(retired);
        Ok(true)
    }
    pub fn configure(&self, settings: VideoTransportSettings, seek: bool) -> Result<(), String> {
        settings.validate()?;
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.settings == settings && !seek {
            return Ok(());
        }
        let play = settings.mode == VideoPlaybackMode::Playing && (state.settings.mode != settings.mode || seek);
        state.seek_pending |= seek || state.settings.position_ms != settings.position_ms;
        state.settings = settings;
        state.apply(play)?;
        drop(state);
        self.1.advance();
        Ok(())
    }
    /// Explicit timing fixture injection; product metadata comes from authored channel bindings.
    #[cfg(test)]
    pub fn prepare(&self, duration: Duration) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let first = state.transport.duration().is_zero();
        state.transport.set_duration(duration).map_err(|e| e.to_string())?;
        state.apply(first)
    }
    pub fn request_play(&self) -> Result<(), String> {
        self.0.lock().map_err(|_| "Video control poisoned")?.apply(true)?;
        self.1.advance();
        Ok(())
    }
    pub fn snapshot(&self, media: super::super::protocol::MediaId) -> Result<VideoTransportSnapshot, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let sample = state.transport.sample().map_err(|e| e.to_string())?;
        Ok(VideoTransportSnapshot {
            media,
            settings: state.settings.clone(),
            ready: !state.transport.duration().is_zero(),
            position_ms: sample.position.as_secs_f64() * 1000.0,
            playing: sample.playing,
        })
    }
    #[cfg(any(feature = "video-files", test))]
    pub(crate) fn observe_neighbours(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        expected: super::TransportSample,
        frame: super::Neighbours,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        if (now.epoch, now.cycle) == (expected.epoch, expected.cycle) {
            state.frames.observe(channel, owner, expected, frame);
        }
        Ok(())
    }
    pub(crate) fn frame(
        &self,
        reference: super::super::protocol::ChannelReference,
        file: &super::super::protocol::AssetLocator,
    ) -> Result<Option<VideoFrameState>, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        Ok(state.frames.inspect(reference.media, reference.channel, file, now))
    }
    pub(crate) fn step_position(
        &self,
        channel: super::super::protocol::ChannelId,
        file: &super::super::protocol::AssetLocator,
        direction: VideoStepDirection,
    ) -> Result<f64, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        state
            .frames
            .step(channel, file, now, direction)
            .map(super::frames::millis)
    }
}
impl State {
    fn duration_changed(&mut self, old: Option<Duration>, new: Option<Duration>) -> Result<(), String> {
        if old == new {
            return Ok(());
        }
        if let Some(old) = old {
            let count = self.durations.get_mut(&old).expect("every bound duration has a count");
            *count -= 1;
            if *count == 0 {
                self.durations.remove(&old);
            }
        }
        if let Some(new) = new {
            *self.durations.entry(new).or_default() += 1;
        }
        let duration = self
            .durations
            .last_key_value()
            .map_or(Duration::ZERO, |(duration, _)| *duration);
        if duration == self.transport.duration() {
            return Ok(());
        }
        let first = self.transport.duration().is_zero();
        let extended = duration > self.transport.duration();
        self.seek_pending |= duration.is_zero();
        self.seek_pending |= extended && self.seek_waiting_for_duration;
        self.transport.set_duration(duration).map_err(|e| e.to_string())?;
        self.apply(first || extended)
    }
    fn apply(&mut self, play_requested: bool) -> Result<(), String> {
        let rate = PlaybackRate::new((self.settings.rate * 1_000_000.0).round() as i32, 1_000_000)
            .map_err(|e| e.to_string())?;
        self.transport.set_rate(rate).map_err(|e| e.to_string())?;
        self.transport
            .set_looping(self.settings.looping)
            .map_err(|e| e.to_string())?;
        if self.settings.mode == VideoPlaybackMode::Stopped {
            self.transport.stop().map_err(|e| e.to_string())?;
            self.seek_pending = false;
            self.seek_waiting_for_duration = false;
            return Ok(());
        }
        if self.transport.duration().is_zero() {
            return Ok(());
        }
        if self.seek_pending {
            // Metadata clamps a saved seek only at the runtime boundary; authored values remain intact.
            let requested = Duration::from_secs_f64(self.settings.position_ms / 1000.0);
            let position = requested.min(self.transport.duration());
            self.transport.seek(position).map_err(|e| e.to_string())?;
            self.seek_pending = false;
            self.seek_waiting_for_duration = requested > self.transport.duration();
        }
        match self.settings.mode {
            VideoPlaybackMode::Playing if play_requested => self.transport.play(),
            VideoPlaybackMode::Playing => Ok(()),
            VideoPlaybackMode::Paused => self.transport.pause(),
            VideoPlaybackMode::Stopped => unreachable!(),
        }
        .map_err(|e| e.to_string())
    }
}
