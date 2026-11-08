use super::{MediaTransport, protocol::*};
use golden_time::{PlaybackRate, SystemClock};
use std::{sync::Mutex, time::Duration};

struct State {
    transport: MediaTransport<SystemClock>,
    settings: VideoTransportSettings,
    seek_pending: bool,
    durations: std::collections::HashMap<super::super::protocol::ChannelId, Duration>,
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
                durations: Default::default(),
                frames: Default::default(),
            }),
            Default::default(),
        )
    }
}
impl VideoControl {
    pub fn claim_channel(
        &self,
        channel: super::super::protocol::ChannelId,
        owner: uuid::Uuid,
        file: super::super::protocol::AssetLocator,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if !state.durations.contains_key(&channel) && state.durations.len() >= 4096 {
            return Err("Video metadata exceeds the 4096-channel runtime capture bound".into());
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
        if !state.frames.owns(channel, owner) {
            return Err("Video source owner was superseded".into());
        }
        state.durations.insert(channel, duration);
        let duration = state.durations.values().copied().max().unwrap_or_default();
        let first = state.transport.duration().is_zero();
        let extended = duration > state.transport.duration();
        state.transport.set_duration(duration).map_err(|e| e.to_string())?;
        state.apply(first || extended)?;
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
    /// The file worker supplies aggregate metadata; transient source loss must retain known duration.
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
            return Ok(());
        }
        if self.transport.duration().is_zero() {
            return Ok(());
        }
        if self.seek_pending {
            // Metadata clamps a saved seek only at the runtime boundary; authored values remain intact.
            let position = Duration::from_secs_f64(self.settings.position_ms / 1000.0).min(self.transport.duration());
            self.transport.seek(position).map_err(|e| e.to_string())?;
            self.seek_pending = false;
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
