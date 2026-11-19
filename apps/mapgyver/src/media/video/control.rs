use super::{MediaTransport, protocol::*};
use golden_time::{PlaybackRate, SystemClock};
use golden_video::scheduling::SourceRange;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[cfg(feature = "video-files")]
mod audio;
mod matrix;
#[cfg(all(windows, feature = "native-video", feature = "video-files"))]
pub(super) mod scheduling;
mod sequence;
#[cfg(test)]
mod tests;
pub use matrix::MatrixVideoUse;

struct State {
    transport: MediaTransport<SystemClock>,
    #[cfg(feature = "video-files")]
    clock: SystemClock,
    settings: VideoTransportSettings,
    seek_pending: bool,
    retired: bool,
    files: std::collections::HashMap<super::super::protocol::ChannelId, super::bindings::File>,
    durations: std::collections::BTreeMap<Duration, usize>,
    source_duration: Duration,
    separate_repeat: bool,
    repeat_duration: Duration,
    shortest_repeat_duration: Option<Duration>,
    frames: super::frames::Frames,
    feedback: Option<super::feedback::Publisher>,
    feedback_sequence: u64,
    input_time: Option<golden_core::engine::EngineTime>,
    duration_changes: golden_io::RevisionSource,
    position_range: golden_core::parameter::RuntimeScalarRange,
    load_pending: bool,
    ready_once: bool,
    explicit_mode: bool,
    manual_revision: u64,
    matrix_uses: matrix::Uses,
    matrix_generation: uuid::Uuid,
    retain_play_position: bool,
    #[cfg(feature = "video-files")]
    matrix_audio_transport: bool,
    #[cfg(feature = "video-files")]
    matrix_owned_audio: bool,
}

/// Shared per-media runtime control, retained by demanded sources rather than per Reference.
pub struct VideoControl(Mutex<State>, golden_io::RevisionSource);
impl Default for VideoControl {
    fn default() -> Self {
        let clock = SystemClock::new();
        Self(
            Mutex::new(State {
                transport: MediaTransport::new(clock.clone()).expect("fresh monotonic clock is representable"),
                #[cfg(feature = "video-files")]
                clock,
                settings: Default::default(),
                seek_pending: true,
                retired: false,
                files: Default::default(),
                durations: Default::default(),
                source_duration: Duration::ZERO,
                separate_repeat: false,
                repeat_duration: Duration::ZERO,
                shortest_repeat_duration: None,
                frames: Default::default(),
                feedback: None,
                feedback_sequence: 0,
                input_time: None,
                duration_changes: Default::default(),
                position_range: golden_core::parameter::RuntimeScalarRange::new(0.0, 0.0).expect("valid zero bounds"),
                load_pending: false,
                ready_once: false,
                explicit_mode: false,
                manual_revision: 0,
                matrix_uses: Default::default(),
                matrix_generation: uuid::Uuid::new_v4(),
                retain_play_position: false,
                #[cfg(feature = "video-files")]
                matrix_audio_transport: false,
                #[cfg(feature = "video-files")]
                matrix_owned_audio: false,
            }),
            Default::default(),
        )
    }
}
impl VideoControl {
    pub(crate) fn position_range(&self) -> Result<golden_core::parameter::RuntimeScalarRange, String> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "Video control poisoned")?
            .position_range
            .clone())
    }
    pub(crate) fn settings(&self) -> Result<VideoTransportSettings, String> {
        Ok(self.0.lock().map_err(|_| "Video control poisoned")?.settings.clone())
    }
    pub(crate) fn source_duration_ms(&self) -> Result<Option<f64>, String> {
        let state = self.0.lock().map_err(|_| "Video control poisoned")?;
        Ok((!state.source_duration.is_zero()).then(|| super::frames::millis(state.source_duration)))
    }
    pub(crate) fn bounded_position(&self, position_ms: f64) -> Result<f64, String> {
        if !position_ms.is_finite() {
            return Err("Current Position must be finite".into());
        }
        let state = self.0.lock().map_err(|_| "Video control poisoned")?;
        Ok(position_ms.clamp(0.0, super::frames::millis(state.transport.duration())))
    }
    pub(crate) fn initialize(&self, mut settings: VideoTransportSettings) -> Result<(), String> {
        settings.validate()?;
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        settings.mode = if settings.play_at_load {
            VideoPlaybackMode::Playing
        } else {
            VideoPlaybackMode::Paused
        };
        state.ready_once = !state.source_duration.is_zero();
        state.load_pending = settings.play_at_load && !state.ready_once;
        state.explicit_mode = false;
        state.retain_play_position = false;
        state.input_time = None;
        state.seek_pending = true;
        state.settings = settings;
        state.apply(true)?;
        self.1.advance();
        drop(state);
        self.publish_feedback()
    }
    pub(crate) fn command(&self, mode: VideoPlaybackMode) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.retired {
            return Err("Video media is retired".into());
        }
        state.manual_revision = state
            .manual_revision
            .checked_add(1)
            .ok_or("Video intent revision exhausted")?;
        state.explicit_mode = true;
        state.retain_play_position = false;
        state.load_pending = false;
        state.settings.mode = mode;
        #[cfg(feature = "video-files")]
        {
            state.matrix_audio_transport = false;
        }
        if mode == VideoPlaybackMode::Stopped {
            state.settings.position_ms = 0.0;
        }
        state.apply(mode == VideoPlaybackMode::Playing)?;
        self.1.advance();
        drop(state);
        self.publish_feedback()
    }
    pub(crate) fn set_feedback(&self, publisher: super::feedback::Publisher) -> Result<(), String> {
        self.0.lock().map_err(|_| "Video control poisoned")?.feedback = Some(publisher);
        self.publish_feedback()
    }
    pub(crate) fn publish_feedback(&self) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.retired {
            return Ok(());
        }
        let Some(publisher) = state.feedback.clone() else {
            return Ok(());
        };
        let sample = state.transport.sample().map_err(|error| error.to_string())?;
        state.feedback_sequence = state
            .feedback_sequence
            .checked_add(1)
            .ok_or("Video status sequence exhausted")?;
        let sequence = state.feedback_sequence;
        let status = super::feedback::Status {
            playing: sample.playing,
            length_ms: super::frames::millis(sample.duration),
            position_ms: super::frames::millis(sample.position),
            input_time: state.input_time,
            range_guard: {
                let reader = state.duration_changes.reader();
                reader.guard_at(reader.current())
            },
            guard: {
                let reader = self.1.reader();
                reader.guard_at(reader.current())
            },
        };
        drop(state);
        publisher.publish(sequence, status);
        Ok(())
    }
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
    pub(super) fn metadata_tracks(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
        tracks: Vec<VideoSubtitleTrackInfo>,
        audio_options: golden_core::parameter::ParameterEnumOptions,
    ) -> Result<(), String> {
        let state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if let Some(file) = state.files.get(&channel).filter(|file| file.generation == generation) {
            file.subtitles.set_tracks(tracks)?;
            file.audio.set_options(audio_options);
        }
        Ok(())
    }
    #[cfg(feature = "video-files")]
    pub(super) fn metadata_failure(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
        error: VideoSourceError,
    ) -> Result<(), String> {
        self.invalidate_file_contents(channel, generation, Some(error))
    }
    #[cfg(feature = "video-files")]
    pub(super) fn invalidate_file_contents(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
        error: Option<VideoSourceError>,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let Some(file) = state
            .files
            .get_mut(&channel)
            .filter(|file| file.generation == generation)
        else {
            return Ok(());
        };
        if error.is_some() && file.metadata_error == error {
            return Ok(());
        }
        // Keep the authored binding and known duration. The old worker cannot republish
        // through its retired frame owner, and audio observes the same content revision.
        file.subtitles.set_tracks(Vec::new())?;
        file.audio.set_options(super::audio_tracks::default_options());
        file.metadata_error = error;
        file.contents.advance();
        state.frames.remove(channel);
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
        if changed {
            self.1.advance();
        }
        drop(state);
        Ok(())
    }
    pub(crate) fn retire_media(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.retired = true;
            state.feedback = None;
            state.matrix_generation = uuid::Uuid::new_v4();
            state.matrix_uses = Default::default();
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
    #[cfg(test)]
    pub(crate) fn bind_file(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        locator: Option<super::super::protocol::AssetLocator>,
    ) -> Result<(), String> {
        self.bind_file_with_settings(
            channel,
            owner,
            locator,
            Arc::default(),
            Arc::default(),
            Default::default(),
        )
    }
    pub(super) fn bind_file_with_settings(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        locator: Option<super::super::protocol::AssetLocator>,
        subtitles: Arc<super::bindings::ChannelSubtitles>,
        audio: Arc<super::audio_tracks::ChannelAudio>,
        loop_files: super::bindings::LoopFiles,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if let Some(file) = state
            .files
            .get_mut(&channel)
            .filter(|file| file.owner == owner && file.locator == locator)
        {
            if file.loop_files != loop_files {
                if file.loop_files.picture != loop_files.picture {
                    file.repeat = loop_files.picture.as_ref().map(|_| super::bindings::RepeatFile::new());
                }
                file.loop_files = loop_files;
                state.update_repeat()?;
                self.1.advance();
            }
            return Ok(());
        }
        if !state.files.contains_key(&channel) && state.files.len() >= 4096 {
            return Err("Video metadata exceeds 4096 authored channels".into());
        }
        subtitles.set_tracks(Vec::new())?;
        audio.set_options(super::audio_tracks::default_options());
        state.frames.remove(channel);
        let old = state.files.insert(
            channel,
            super::bindings::File {
                owner,
                #[cfg(any(feature = "video-files", test))]
                generation: uuid::Uuid::new_v4(),
                contents: Default::default(),
                locator,
                duration: None,
                subtitles,
                #[cfg(feature = "video-files")]
                audio,
                metadata_error: None,
                repeat: loop_files.picture.as_ref().map(|_| super::bindings::RepeatFile::new()),
                loop_files,
            },
        );
        state.update_repeat()?;
        state.duration_changed(old.and_then(|file| file.duration), None)?;
        self.1.advance();
        drop(state);
        Ok(())
    }
    pub(crate) fn unbind_file(&self, channel: super::super::protocol::ChannelId, owner: uuid::Uuid) {
        if let Ok(mut state) = self.0.lock()
            && !state.retired
            && state.files.get(&channel).is_some_and(|file| file.owner == owner)
        {
            let old = state.files.remove(&channel).and_then(|file| file.duration);
            state.frames.remove(channel);
            let _ = state.update_repeat();
            let _ = state.duration_changed(old, None);
            self.1.advance();
            drop(state);
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
            .flat_map(|(channel, file)| {
                let intro = file.locator.clone().map(|locator| (*channel, file.generation, locator));
                let repeat = intro
                    .as_ref()
                    .and_then(|_| Some((*channel, file.repeat?.generation, file.loop_files.picture.clone()?)));
                [intro, repeat].into_iter().flatten()
            })
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
        if let Some(repeat) = state
            .files
            .get_mut(&channel)
            .and_then(|file| file.repeat.as_mut())
            .filter(|file| file.generation == generation)
        {
            if repeat.duration != Some(duration) {
                repeat.duration = Some(duration);
                state.update_repeat()?;
                self.1.advance();
            }
            return Ok(true);
        }
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
                self.1.advance();
                drop(state);
            }
            return Ok(true);
        }
        let old = file.duration.replace(duration);
        state.duration_changed(old, Some(duration))?;
        self.1.advance();
        drop(state);
        Ok(true)
    }
    #[cfg(feature = "video-files")]
    pub(super) fn metadata_is_current(
        &self,
        channel: super::super::protocol::ChannelId,
        generation: uuid::Uuid,
    ) -> bool {
        self.0.lock().is_ok_and(|state| {
            state.files.get(&channel).is_some_and(|file| {
                file.generation == generation || file.repeat.is_some_and(|file| file.generation == generation)
            })
        })
    }
    pub(crate) fn claim_channel(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        file: super::super::protocol::AssetLocator,
    ) -> Result<(Arc<super::bindings::ChannelSubtitles>, golden_io::RevisionGuard), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let binding = state
            .files
            .get(&channel)
            .filter(|binding| binding.locator.as_ref() == Some(&file))
            .ok_or("Video source no longer matches its authored File")?;
        let subtitles = binding.subtitles.clone();
        let contents = binding.contents.reader();
        let contents = contents.guard_at(contents.current());
        state.frames.claim(channel, owner, file);
        Ok((subtitles, contents))
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
        self.1.advance();
        drop(state);
        Ok(())
    }
    pub fn release_channel(&self, channel: super::super::protocol::ChannelId, owner: uuid::Uuid) {
        if let Ok(mut state) = self.0.lock() {
            // Demand belongs to a consumer, not to media transport. Keep known metadata
            // while previews/References are closed; resubscription must not reset time.
            state.frames.release(channel, owner);
        }
    }
    pub(crate) fn publish_frame(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        expected: super::TransportSample,
        stream: &golden_video::image::ImageStream,
        image: golden_video::image::ImageData,
        frame: super::frames::Neighbours,
    ) -> Result<bool, String> {
        let window = super::frames::loop_window(expected, frame)?;
        self.publish_with(channel, owner, expected, || stream.replace_frame(image, window))
    }
    fn publish_with<T>(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        expected: super::TransportSample,
        publish: impl FnOnce() -> Result<T, &'static str>,
    ) -> Result<bool, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        if !state.frames.owns(channel, owner) || (now.epoch, now.cycle) != (expected.epoch, expected.cycle) {
            return Ok(false);
        }
        let retired = publish().map_err(str::to_owned)?;
        state.frames.ready(channel, owner);
        drop(state);
        drop(retired);
        Ok(true)
    }
    pub fn configure(
        &self,
        mut settings: VideoTransportSettings,
        seek: bool,
        input_time: Option<golden_core::engine::EngineTime>,
    ) -> Result<(), String> {
        settings.validate()?;
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.settings == settings && !seek {
            return Ok(());
        }
        let explicit = state.settings.mode != settings.mode;
        #[cfg(feature = "video-files")]
        if explicit {
            state.matrix_audio_transport = false;
        }
        if seek
            || explicit
            || state.settings.position_ms != settings.position_ms
            || state.settings.start_ms != settings.start_ms
            || state.settings.end_ms != settings.end_ms
            || state.settings.speed != settings.speed
            || state.settings.looping != settings.looping
            || (settings.looping
                && (state.settings.auto_smooth_loop_ms != settings.auto_smooth_loop_ms
                    || state.settings.loop_start_ms != settings.loop_start_ms))
            || state.settings.play_at_load != settings.play_at_load
        {
            state.manual_revision = state
                .manual_revision
                .checked_add(1)
                .ok_or("Video intent revision exhausted")?;
        }
        if explicit || state.settings.play_at_load != settings.play_at_load {
            state.retain_play_position = false;
        }
        if seek || state.settings.position_ms != settings.position_ms {
            // Seek selects an exact coordinate; only an explicit Play rewinds terminal edges.
            state.retain_play_position = true;
        }
        if explicit {
            state.explicit_mode = true;
            state.load_pending = false;
        } else if settings.play_at_load != state.settings.play_at_load {
            if !settings.play_at_load && state.load_pending {
                state.load_pending = false;
                settings.mode = VideoPlaybackMode::Paused;
            } else if settings.play_at_load && !state.ready_once && !state.explicit_mode {
                state.load_pending = true;
                settings.mode = VideoPlaybackMode::Playing;
            }
        }
        let play = settings.mode == VideoPlaybackMode::Playing && (state.settings.mode != settings.mode || seek);
        if let Some(time) = input_time {
            state.input_time = Some(time);
        }
        state.seek_pending |= seek || state.settings.position_ms != settings.position_ms;
        state.settings = settings;
        state.apply(play)?;
        self.1.advance();
        drop(state);
        self.publish_feedback()
    }
    /// Explicit timing fixture injection; product metadata comes from authored channel bindings.
    #[cfg(test)]
    pub fn prepare(&self, duration: Duration) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let first = state.source_duration.is_zero();
        state.source_duration = duration;
        state.ready_once |= !duration.is_zero();
        state.load_pending = false;
        state.apply(first)?;
        self.1.advance();
        drop(state);
        self.publish_feedback()
    }
    pub fn snapshot(&self, media: super::super::protocol::MediaId) -> Result<VideoTransportSnapshot, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let sample = state.transport.sample().map_err(|e| e.to_string())?;
        Ok(VideoTransportSnapshot {
            media,
            settings: state.settings.clone(),
            ready: !state.transport.duration().is_zero(),
            length_ms: (!state.source_duration.is_zero()).then(|| super::frames::millis(sample.duration)),
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
    /// None means a paused boundary Step: preserve its prepared neighbours and clock epoch.
    pub(crate) fn step_position(
        &self,
        channel: super::super::protocol::ChannelId,
        file: &super::super::protocol::AssetLocator,
        direction: VideoStepDirection,
    ) -> Result<Option<f64>, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let now = state.transport.sample().map_err(|e| e.to_string())?;
        let position = state.frames.step(channel, file, now, direction)?;
        Ok(
            (state.settings.mode != VideoPlaybackMode::Paused || position != now.position)
                .then(|| super::frames::millis(position)),
        )
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
        if duration == self.source_duration {
            return self.update_loop_policy();
        }
        let extended = duration > self.source_duration;
        self.source_duration = duration;
        self.seek_pending |= duration.is_zero();
        if !duration.is_zero() {
            self.ready_once = true;
            self.load_pending = false;
        }
        self.apply(extended)
    }
    fn apply(&mut self, play_requested: bool) -> Result<(), String> {
        let previously_empty = self.transport.duration().is_zero();
        self.update_source_range()?;
        let play_requested = play_requested || previously_empty;
        // Accepted transport positions never wait outside the currently known timeline.
        self.settings.position_ms = self
            .settings
            .position_ms
            .min(super::frames::millis(self.transport.duration()));
        let rate = PlaybackRate::new((self.settings.speed * 1_000_000.0).round() as i32, 1_000_000)
            .map_err(|e| e.to_string())?;
        self.transport.set_rate(rate).map_err(|e| e.to_string())?;
        self.update_loop_policy()?;
        if self.settings.mode == VideoPlaybackMode::Stopped {
            self.transport.stop().map_err(|e| e.to_string())?;
            self.seek_pending = false;
            return Ok(());
        }
        if self.transport.duration().is_zero() {
            return Ok(());
        }
        if self.seek_pending {
            let position = Duration::from_secs_f64(self.settings.position_ms / 1000.0);
            self.transport.seek(position).map_err(|e| e.to_string())?;
            self.seek_pending = false;
        }
        match self.settings.mode {
            VideoPlaybackMode::Playing if play_requested && self.retain_play_position => {
                self.transport.resume_position()
            }
            VideoPlaybackMode::Playing if play_requested => self.transport.play(),
            VideoPlaybackMode::Playing => Ok(()),
            VideoPlaybackMode::Paused => self.transport.pause(),
            VideoPlaybackMode::Stopped => unreachable!(),
        }
        .map_err(|e| e.to_string())
    }

    fn update_source_range(&mut self) -> Result<(), String> {
        let (start, intro_end) = self.intro_bounds();
        let end = if self.settings.looping && self.separate_repeat && intro_end > start {
            intro_end
                .saturating_add(self.repeat_duration)
                .min(Duration::from_nanos(i64::MAX as u64))
        } else {
            intro_end
        };
        let range = SourceRange::new(start, end);
        if range != self.transport.source_range() {
            self.transport
                .set_source_range(range)
                .map_err(|error| error.to_string())?;
            self.position_range
                .set(0.0, super::frames::millis(self.transport.duration()))?;
            self.duration_changes.advance();
        }
        Ok(())
    }

    fn intro_bounds(&self) -> (Duration, Duration) {
        let start = Duration::from_secs_f64(self.settings.start_ms / 1_000.0);
        let end = self.settings.end_ms.map_or(self.source_duration, |end| {
            Duration::from_secs_f64(end / 1_000.0).min(self.source_duration)
        });
        (start, end)
    }

    fn update_loop_policy(&mut self) -> Result<(), String> {
        let shortest = self
            .durations
            .first_key_value()
            .map_or(self.source_duration, |(shortest, _)| *shortest);
        let (looping, start, overlap) =
            self.transport
                .source_range()
                .map_or((self.settings.looping, Duration::ZERO, Duration::ZERO), |range| {
                    if self.separate_repeat {
                        let intro_end = self.intro_bounds().1;
                        let tail = range.end().saturating_sub(intro_end);
                        let overlap = Duration::from_secs_f64(self.settings.auto_smooth_loop_ms / 1_000.0)
                            .min(self.shortest_repeat_duration.unwrap_or(tail) / 2)
                            .min(tail / 2);
                        return (
                            self.settings.looping && !tail.is_zero(),
                            intro_end - range.start(),
                            overlap,
                        );
                    }
                    let source_start =
                        Duration::from_secs_f64(self.settings.loop_start_ms / 1_000.0).max(range.start());
                    let start = source_start - range.start();
                    let shared_tail = range.end().min(shortest).saturating_sub(source_start);
                    let overlap =
                        Duration::from_secs_f64(self.settings.auto_smooth_loop_ms / 1_000.0).min(shared_tail / 2);
                    (self.settings.looping && start < range.duration(), start, overlap)
                });
        self.transport
            .set_loop_section(looping, start, overlap)
            .map_err(|error| error.to_string())
    }
}
