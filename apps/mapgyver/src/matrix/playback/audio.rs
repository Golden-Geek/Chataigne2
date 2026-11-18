use super::{Control, PlaybackControl, State};
use crate::matrix::audio::Mix;
use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
use std::{
    sync::{Arc, Weak, atomic::Ordering},
    time::Instant,
};

impl State {
    pub(super) fn track_audio(&mut self, plan: &crate::matrix::video::Prepared, now: Instant) {
        self.audio.track(plan, &self.timeline, now);
    }
}

#[derive(Clone)]
pub(crate) struct AudioReader(Weak<Control>);

impl AudioReader {
    pub fn upgrade(&self) -> Option<PlaybackControl> {
        let control = self.0.upgrade()?;
        control
            .audio_active
            .load(Ordering::Acquire)
            .then_some(PlaybackControl(control))
    }

    pub fn alive(&self) -> bool {
        self.0.strong_count() != 0
    }
}

impl PlaybackControl {
    pub(crate) fn audio_reader(&self) -> AudioReader {
        AudioReader(Arc::downgrade(&self.0))
    }

    pub(crate) fn capture_audio(&self, now: Instant, mix: &mut Mix) -> Result<(), String> {
        self.snapshot();
        let mut state = self.0.state.lock().unwrap();
        if state.audio.is_empty() || !state.video_enabled {
            self.0
                .audio_active
                .store(state.pending.is_some() || !state.audio.is_empty(), Ordering::Release);
            return Ok(());
        }
        let time = if let Some(observation) = &state.video_time {
            observation.project(now)?
        } else {
            let clock = state.audio_clock.get_or_insert_with(|| {
                TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
                    .expect("new Matrix audio clock")
            });
            clock.sample().map_err(|error| error.to_string())?
        };
        let version = state.version;
        let desired = state.current.clone();
        let State {
            timeline,
            epochs,
            launch_orders,
            cuts,
            audio,
            videos,
            ..
        } = &mut *state;
        if let Some(frame) = timeline.frame(&desired, Some(time), version, epochs, launch_orders, cuts)? {
            *cuts = Default::default();
            videos.retain_frame(&frame);
            audio.capture(&frame, now, time, mix);
        }
        self.0
            .audio_active
            .store(state.pending.is_some() || !state.audio.is_empty(), Ordering::Release);
        Ok(())
    }
}
