use crate::{AudioError, AudioErrorCategory, FrameCount, SampleRate};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClockSource {
    Null,
    Output(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockHandoffPhase {
    Stable,
    FadingDown,
    FadingUp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockAuthority {
    pub source: ClockSource,
    pub phase: ClockHandoffPhase,
    pub pending_output: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockBlock {
    pub start_frame: u64,
    pub end_frame: u64,
    pub source: ClockSource,
    pub render: bool,
    pub gain_start: f32,
    pub gain_end: f32,
}

#[derive(Clone, Debug)]
pub struct RenderClockCoordinator {
    sample_rate: SampleRate,
    frame: u64,
    authority: ClockSource,
    phase: ClockHandoffPhase,
    pending_output: Option<u64>,
    fade_frames: u32,
    fade_progress: u32,
    retired_outputs: u64,
}

impl RenderClockCoordinator {
    pub fn new(sample_rate: SampleRate, fade_frames: FrameCount) -> Result<Self, AudioError> {
        Ok(Self {
            sample_rate,
            frame: 0,
            authority: ClockSource::Null,
            phase: ClockHandoffPhase::Stable,
            pending_output: None,
            fade_frames: fade_frames.get(),
            fade_progress: 0,
            retired_outputs: 0,
        })
    }

    pub fn prime_output(&mut self, generation: u64) -> Result<(), AudioError> {
        if generation == 0 {
            return Err(AudioError::invalid_configuration(
                "output stream generation must be greater than zero",
            ));
        }
        if self.authority == ClockSource::Output(generation) {
            return Ok(());
        }
        self.pending_output = Some(generation);
        self.fade_progress = 0;
        self.phase = match self.authority {
            ClockSource::Null => ClockHandoffPhase::FadingUp,
            ClockSource::Output(_) => ClockHandoffPhase::FadingDown,
        };
        if self.authority == ClockSource::Null {
            self.authority = ClockSource::Output(generation);
            self.pending_output = None;
        }
        Ok(())
    }

    pub fn output_lost(&mut self, generation: u64) {
        if self.authority == ClockSource::Output(generation) {
            self.authority = self
                .pending_output
                .take()
                .map_or(ClockSource::Null, ClockSource::Output);
            self.phase = if self.authority == ClockSource::Null {
                ClockHandoffPhase::Stable
            } else {
                ClockHandoffPhase::FadingUp
            };
            self.fade_progress = 0;
            self.retired_outputs = self.retired_outputs.saturating_add(1);
        }
        if self.pending_output == Some(generation) {
            self.pending_output = None;
        }
    }

    pub fn advance(&mut self, source: ClockSource, frames: FrameCount) -> Result<ClockBlock, AudioError> {
        let start_frame = self.frame;
        if source != self.authority {
            return Ok(ClockBlock {
                start_frame,
                end_frame: start_frame,
                source,
                render: false,
                gain_start: 0.0,
                gain_end: 0.0,
            });
        }
        let (gain_start, gain_end) = self.block_gain(frames.get());
        self.frame = self.frame.checked_add(u64::from(frames.get())).ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::CapacityExceeded,
                "render clock frame counter overflowed",
            )
        })?;
        self.finish_phase_if_ready();
        Ok(ClockBlock {
            start_frame,
            end_frame: self.frame,
            source,
            render: true,
            gain_start,
            gain_end,
        })
    }

    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    #[must_use]
    pub const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[must_use]
    pub const fn authority(&self) -> ClockAuthority {
        ClockAuthority {
            source: self.authority,
            phase: self.phase,
            pending_output: self.pending_output,
        }
    }

    #[must_use]
    pub const fn retired_outputs(&self) -> u64 {
        self.retired_outputs
    }

    fn block_gain(&mut self, frames: u32) -> (f32, f32) {
        match self.phase {
            ClockHandoffPhase::Stable => (1.0, 1.0),
            ClockHandoffPhase::FadingDown => {
                let start = 1.0 - self.fade_fraction();
                self.fade_progress = self.fade_progress.saturating_add(frames).min(self.fade_frames);
                (start, 1.0 - self.fade_fraction())
            }
            ClockHandoffPhase::FadingUp => {
                let start = self.fade_fraction();
                self.fade_progress = self.fade_progress.saturating_add(frames).min(self.fade_frames);
                (start, self.fade_fraction())
            }
        }
    }

    fn fade_fraction(&self) -> f32 {
        self.fade_progress as f32 / self.fade_frames as f32
    }

    fn finish_phase_if_ready(&mut self) {
        if self.fade_progress < self.fade_frames {
            return;
        }
        match self.phase {
            ClockHandoffPhase::FadingDown => {
                let old = self.authority;
                let Some(next) = self.pending_output.take() else {
                    self.authority = ClockSource::Null;
                    self.phase = ClockHandoffPhase::Stable;
                    self.fade_progress = 0;
                    return;
                };
                self.authority = ClockSource::Output(next);
                self.phase = ClockHandoffPhase::FadingUp;
                self.fade_progress = 0;
                if matches!(old, ClockSource::Output(_)) {
                    self.retired_outputs = self.retired_outputs.saturating_add(1);
                }
            }
            ClockHandoffPhase::FadingUp => {
                self.phase = ClockHandoffPhase::Stable;
                self.fade_progress = 0;
            }
            ClockHandoffPhase::Stable => {}
        }
    }
}
