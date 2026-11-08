use super::{MediaTransport, protocol::*};
use golden_time::{PlaybackRate, SystemClock};
use std::{sync::Mutex, time::Duration};

/// Coalesced selected-channel step. A worker must resolve its actual timestamp before applying it.
#[derive(Clone, Copy)]
pub struct StepRequest {
    pub revision: u64,
    pub channel: super::super::protocol::ChannelId,
    pub direction: VideoStepDirection,
}
struct State {
    transport: MediaTransport<SystemClock>,
    settings: VideoTransportSettings,
    revision: u64,
    seek_pending: bool,
    step: Option<StepRequest>,
}

/// Shared per-media runtime control, retained by demanded sources rather than per Reference.
pub struct VideoControl(Mutex<State>);
impl Default for VideoControl {
    fn default() -> Self {
        Self(Mutex::new(State {
            transport: MediaTransport::new(SystemClock::new()).expect("fresh monotonic clock is representable"),
            settings: Default::default(),
            revision: 0,
            seek_pending: true,
            step: None,
        }))
    }
}
impl VideoControl {
    pub fn configure(&self, settings: VideoTransportSettings, seek: bool) -> Result<(), String> {
        settings.validate()?;
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.settings == settings && !seek {
            return Ok(());
        }
        let revision = state
            .revision
            .checked_add(1)
            .ok_or("Video control revision exhausted")?;
        let play = settings.mode == VideoPlaybackMode::Playing && (state.settings.mode != settings.mode || seek);
        state.seek_pending |= seek || state.settings.position_ms != settings.position_ms;
        state.settings = settings;
        state.revision = revision;
        state.step = None;
        state.apply(play)
    }
    /// The file worker supplies aggregate metadata; transient source loss must retain known duration.
    pub fn prepare(&self, duration: Duration) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        let first = state.transport.duration().is_zero();
        state.transport.set_duration(duration).map_err(|e| e.to_string())?;
        state.apply(first)
    }
    pub fn request_play(&self) -> Result<(), String> {
        self.0.lock().map_err(|_| "Video control poisoned")?.apply(true)
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
            pending_step: state.step.is_some(),
        })
    }
    pub fn request_step(
        &self,
        channel: super::super::protocol::ChannelId,
        direction: VideoStepDirection,
    ) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or("Video control revision exhausted")?;
        state.step = Some(StepRequest {
            revision: state.revision,
            channel,
            direction,
        });
        Ok(())
    }
    pub fn pending_step(&self) -> Result<Option<StepRequest>, String> {
        Ok(self.0.lock().map_err(|_| "Video control poisoned")?.step)
    }
    /// Reject a superseded seek/step result before it can move the shared timeline.
    pub fn resolve_step(&self, revision: u64, timestamp: Duration) -> Result<bool, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.step.is_none_or(|step| step.revision != revision) {
            return Ok(false);
        }
        state
            .transport
            .step_to_timestamp(timestamp)
            .map_err(|e| e.to_string())?;
        state.step = None;
        Ok(true)
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
