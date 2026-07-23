use std::{path::PathBuf, sync::Arc};

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::{
    AudioError, GainDb, GainSmoother, PlanarBuffer, PlaybackId, PlaybackStopReason, PreparedVoice,
    RealtimeVoiceRetirement, RealtimeVoiceSlots, RetiredVoice, VoiceId, VoiceRetirementReason, VoiceSlotController,
    assert_not_realtime, voice_slot_pool,
};

use super::{ResidentAudioAsset, StreamPlaybackReader};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefaultPlaybackRoute {
    pub source_channel: u16,
    pub destination_channel: u16,
    pub gain: f32,
}

#[must_use]
pub fn default_playback_routes(source_channels: u16, destination_channels: u16) -> Vec<DefaultPlaybackRoute> {
    if source_channels == 0 || destination_channels == 0 {
        return Vec::new();
    }
    if source_channels == 1 {
        if destination_channels >= 2 {
            const MINUS_THREE_DB: f32 = 0.707_106_77;
            return vec![
                DefaultPlaybackRoute {
                    source_channel: 0,
                    destination_channel: 0,
                    gain: MINUS_THREE_DB,
                },
                DefaultPlaybackRoute {
                    source_channel: 0,
                    destination_channel: 1,
                    gain: MINUS_THREE_DB,
                },
            ];
        }
        return vec![DefaultPlaybackRoute {
            source_channel: 0,
            destination_channel: 0,
            gain: 1.0,
        }];
    }
    (0..source_channels.min(destination_channels))
        .map(|channel| DefaultPlaybackRoute {
            source_channel: channel,
            destination_channel: channel,
            gain: 1.0,
        })
        .collect()
}

#[derive(Debug)]
pub enum PlaybackVoiceSource {
    Resident(Arc<ResidentAudioAsset>),
    Stream {
        reader: StreamPlaybackReader,
        channels: u16,
        scratch: Box<[f32]>,
    },
}

impl PlaybackVoiceSource {
    #[must_use]
    pub fn channels(&self) -> u16 {
        match self {
            Self::Resident(asset) => asset.channels(),
            Self::Stream { channels, .. } => *channels,
        }
    }
}

#[derive(Debug)]
pub struct PlaybackVoice {
    playback_id: PlaybackId,
    path: PathBuf,
    source: PlaybackVoiceSource,
    routes: Box<[DefaultPlaybackRoute]>,
    gain: GainSmoother,
    ramp_frames: u32,
    playhead: usize,
    rendered_frames: u64,
    stop_reason: Option<PlaybackStopReason>,
}

impl PlaybackVoice {
    pub fn new(
        playback_id: PlaybackId,
        path: PathBuf,
        source: PlaybackVoiceSource,
        gain: GainDb,
        routes: Vec<DefaultPlaybackRoute>,
        ramp_frames: u32,
        maximum_block_frames: usize,
    ) -> Result<Self, AudioError> {
        assert_not_realtime("playback voice preparation");
        if ramp_frames == 0 || maximum_block_frames == 0 {
            return Err(AudioError::invalid_configuration(
                "playback voice ramp and maximum block size must be greater than zero",
            ));
        }
        let channels = source.channels();
        if routes
            .iter()
            .any(|route| route.source_channel >= channels || !route.gain.is_finite() || route.gain < 0.0)
        {
            return Err(AudioError::invalid_configuration(
                "playback voice contains an invalid source route",
            ));
        }
        if let PlaybackVoiceSource::Stream { scratch, channels, .. } = &source
            && scratch.len() < maximum_block_frames.saturating_mul(usize::from(*channels))
        {
            return Err(AudioError::invalid_configuration(
                "stream playback scratch is smaller than the maximum render block",
            ));
        }
        let target_gain = gain.to_linear();
        let mut smoother = GainSmoother::settled(0.0);
        smoother.set_target(target_gain, ramp_frames);
        Ok(Self {
            playback_id,
            path,
            source,
            routes: routes.into_boxed_slice(),
            gain: smoother,
            ramp_frames,
            playhead: 0,
            rendered_frames: 0,
            stop_reason: None,
        })
    }

    pub fn request_stop(&mut self, reason: PlaybackStopReason) {
        if self.stop_reason.is_none() {
            self.stop_reason = Some(reason);
            self.gain.set_target(0.0, self.ramp_frames);
            if let PlaybackVoiceSource::Stream { reader, .. } = &self.source {
                reader.cancel();
            }
        }
    }

    #[must_use]
    pub const fn playback_id(&self) -> &PlaybackId {
        &self.playback_id
    }

    #[must_use]
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }

    #[must_use]
    pub const fn rendered_frames(&self) -> u64 {
        self.rendered_frames
    }

    fn render(&mut self, destination: &mut PlanarBuffer, frames: usize) -> VoiceRenderOutcome {
        let available = match &mut self.source {
            PlaybackVoiceSource::Resident(asset) => {
                let available = asset.frames().saturating_sub(self.playhead).min(frames);
                for frame in 0..available {
                    let gain = self.gain.next_gain();
                    for route in &self.routes {
                        let sample = asset.sample(usize::from(route.source_channel), self.playhead + frame);
                        let current = destination.sample(usize::from(route.destination_channel), frame);
                        destination.set_sample(
                            usize::from(route.destination_channel),
                            frame,
                            current + sample * route.gain * gain,
                        );
                    }
                }
                self.playhead = self.playhead.saturating_add(available);
                available
            }
            PlaybackVoiceSource::Stream {
                reader,
                channels,
                scratch,
            } => {
                let sample_count = frames.saturating_mul(usize::from(*channels));
                let available = reader.read_interleaved(&mut scratch[..sample_count]);
                for frame in 0..frames {
                    let gain = self.gain.next_gain();
                    for route in &self.routes {
                        let sample = scratch[frame * usize::from(*channels) + usize::from(route.source_channel)];
                        let current = destination.sample(usize::from(route.destination_channel), frame);
                        destination.set_sample(
                            usize::from(route.destination_channel),
                            frame,
                            current + sample * route.gain * gain,
                        );
                    }
                }
                self.playhead = self.playhead.saturating_add(available);
                available
            }
        };
        self.rendered_frames = self
            .rendered_frames
            .saturating_add(u64::try_from(available).unwrap_or(u64::MAX));
        if self.stop_reason.is_some() && self.gain.remaining() == 0 {
            return VoiceRenderOutcome::Stopped;
        }
        match &self.source {
            PlaybackVoiceSource::Resident(asset) if self.playhead >= asset.frames() => VoiceRenderOutcome::Completed,
            PlaybackVoiceSource::Stream { reader, .. } if reader.is_finished() => {
                if reader.state().failed {
                    VoiceRenderOutcome::Failed
                } else {
                    VoiceRenderOutcome::Completed
                }
            }
            PlaybackVoiceSource::Resident(_) | PlaybackVoiceSource::Stream { .. } => VoiceRenderOutcome::Active,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VoiceRenderOutcome {
    Active,
    Completed,
    Stopped,
    Failed,
}

#[derive(Debug)]
enum PlaybackRenderCommand {
    Activate(PreparedVoice<PlaybackVoice>),
    Stop { voice: VoiceId, reason: PlaybackStopReason },
    StopAll { reason: PlaybackStopReason },
}

#[derive(Debug)]
pub enum PlaybackRenderEvent {
    Finished {
        playback_id: PlaybackId,
        path: PathBuf,
        voice: VoiceId,
        rendered_frames: u64,
    },
    Stopped {
        playback_id: PlaybackId,
        path: PathBuf,
        voice: VoiceId,
        reason: PlaybackStopReason,
        rendered_frames: u64,
    },
    Failed {
        playback_id: PlaybackId,
        path: PathBuf,
        voice: VoiceId,
        rendered_frames: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlaybackRenderMetrics {
    pub active_voices: usize,
    pub completed_voices: u64,
    pub stopped_voices: u64,
    pub failed_voices: u64,
    pub command_queue_full: u64,
}

#[derive(Debug)]
pub struct PlaybackVoiceController {
    voices: VoiceSlotController<PlaybackVoice>,
    commands: Producer<PlaybackRenderCommand>,
    reserved: Vec<bool>,
    command_capacity: usize,
    command_queue_full: u64,
    destination_channels: u16,
}

impl PlaybackVoiceController {
    pub fn try_activate(&mut self, voice: PlaybackVoice) -> Result<VoiceId, AudioError> {
        assert_not_realtime("playback voice admission");
        if voice
            .routes
            .iter()
            .any(|route| route.destination_channel >= self.destination_channels)
        {
            return Err(AudioError::invalid_configuration(
                "playback route destination is outside the prepared voice pool",
            ));
        }
        let slot = self
            .reserved
            .iter()
            .position(|reserved| !reserved)
            .ok_or_else(|| AudioError::capacity_exceeded("all playback voice slots are reserved"))?;
        let slot = u16::try_from(slot).map_err(|_| AudioError::capacity_exceeded("voice slot index overflowed"))?;
        let id = self.voices.next_id(slot)?;
        let command = PlaybackRenderCommand::Activate(PreparedVoice {
            id,
            payload: Box::new(voice),
        });
        if let Err(PushError::Full(_command)) = self.commands.push(command) {
            self.command_queue_full = self.command_queue_full.saturating_add(1);
            return Err(AudioError::queue_full("playback render command"));
        }
        self.reserved[usize::from(slot)] = true;
        Ok(id)
    }

    pub fn try_stop(&mut self, voice: VoiceId, reason: PlaybackStopReason) -> Result<(), AudioError> {
        assert_not_realtime("playback voice stop request");
        self.try_command(PlaybackRenderCommand::Stop { voice, reason })
    }

    pub fn try_stop_all(&mut self, reason: PlaybackStopReason) -> Result<(), AudioError> {
        assert_not_realtime("playback voice stop-all request");
        self.try_command(PlaybackRenderCommand::StopAll { reason })
    }

    pub fn reclaim(&mut self, mut consume: impl FnMut(PlaybackRenderEvent)) -> usize {
        let reserved = &mut self.reserved;
        self.voices.reclaim(|retired| {
            reserved[usize::from(retired.id.slot())] = false;
            consume(event_from_retired(retired));
        })
    }

    #[must_use]
    pub const fn command_capacity(&self) -> usize {
        self.command_capacity
    }

    #[must_use]
    pub const fn command_queue_full(&self) -> u64 {
        self.command_queue_full
    }

    fn try_command(&mut self, command: PlaybackRenderCommand) -> Result<(), AudioError> {
        if let Err(PushError::Full(_command)) = self.commands.push(command) {
            self.command_queue_full = self.command_queue_full.saturating_add(1);
            return Err(AudioError::queue_full("playback render command"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlaybackVoiceRenderer {
    voices: RealtimeVoiceSlots<PlaybackVoice>,
    commands: Consumer<PlaybackRenderCommand>,
    retained_activation: Option<PreparedVoice<PlaybackVoice>>,
    destination_channels: usize,
    maximum_block_frames: usize,
    completed_voices: u64,
    stopped_voices: u64,
    failed_voices: u64,
}

impl PlaybackVoiceRenderer {
    pub fn render(&mut self, destination: &mut PlanarBuffer, frames: usize) -> Result<(), AudioError> {
        if destination.channels() < self.destination_channels
            || frames > self.maximum_block_frames
            || frames > destination.frame_capacity()
        {
            return Err(AudioError::invalid_configuration(
                "playback render destination does not match the prepared pool",
            ));
        }
        destination.zero(frames)?;
        self.apply_commands();
        for slot in 0..self.voices.capacity() {
            let Some(id) = self.voices.active_id_at(slot) else {
                continue;
            };
            let outcome = self
                .voices
                .active_mut(id)
                .map_or(VoiceRenderOutcome::Active, |voice| voice.render(destination, frames));
            match outcome {
                VoiceRenderOutcome::Active => {}
                VoiceRenderOutcome::Completed => {
                    self.completed_voices = self.completed_voices.saturating_add(1);
                    self.voices.retire(id, VoiceRetirementReason::Completed);
                }
                VoiceRenderOutcome::Stopped => {
                    self.stopped_voices = self.stopped_voices.saturating_add(1);
                    self.voices.retire(id, VoiceRetirementReason::Requested);
                }
                VoiceRenderOutcome::Failed => {
                    self.failed_voices = self.failed_voices.saturating_add(1);
                    self.voices.retire(id, VoiceRetirementReason::Completed);
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn metrics(&self) -> PlaybackRenderMetrics {
        PlaybackRenderMetrics {
            active_voices: (0..self.voices.capacity())
                .filter(|slot| self.voices.active_id_at(*slot).is_some())
                .count(),
            completed_voices: self.completed_voices,
            stopped_voices: self.stopped_voices,
            failed_voices: self.failed_voices,
            command_queue_full: 0,
        }
    }

    /// Transfers every callback-owned voice and queued activation for destruction off the callback.
    #[must_use]
    pub fn into_retirement(self) -> PlaybackRendererRetirement {
        PlaybackRendererRetirement {
            voices: self.voices.into_retirement(),
            commands: self.commands,
            retained_activation: self.retained_activation,
        }
    }

    fn apply_commands(&mut self) {
        self.voices.flush_retired();
        if let Some(prepared) = self.retained_activation.take()
            && let Err(prepared) = self.voices.activate(prepared)
        {
            self.retained_activation = Some(prepared);
            return;
        }
        while let Ok(command) = self.commands.pop() {
            match command {
                PlaybackRenderCommand::Activate(prepared) => {
                    if let Err(prepared) = self.voices.activate(prepared) {
                        self.retained_activation = Some(prepared);
                        break;
                    }
                }
                PlaybackRenderCommand::Stop { voice, reason } => {
                    if let Some(active) = self.voices.active_mut(voice) {
                        active.request_stop(reason);
                    }
                }
                PlaybackRenderCommand::StopAll { reason } => {
                    for slot in 0..self.voices.capacity() {
                        if let Some(id) = self.voices.active_id_at(slot)
                            && let Some(active) = self.voices.active_mut(id)
                        {
                            active.request_stop(reason);
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct PlaybackRendererRetirement {
    voices: RealtimeVoiceRetirement<PlaybackVoice>,
    commands: Consumer<PlaybackRenderCommand>,
    retained_activation: Option<PreparedVoice<PlaybackVoice>>,
}

impl PlaybackRendererRetirement {
    pub fn reclaim(mut self) {
        assert_not_realtime("final playback renderer reclamation");
        while self.commands.pop().is_ok() {}
        drop(self.retained_activation.take());
        self.voices.reclaim();
    }
}

pub fn playback_voice_pool(
    capacity: u16,
    destination_channels: u16,
    maximum_block_frames: usize,
) -> Result<(PlaybackVoiceController, PlaybackVoiceRenderer), AudioError> {
    if destination_channels == 0 || maximum_block_frames == 0 {
        return Err(AudioError::invalid_configuration(
            "playback voice pool requires positive destination channels and block capacity",
        ));
    }
    let (voices, realtime_voices) = voice_slot_pool(capacity)?;
    let command_capacity = usize::from(capacity).saturating_mul(3).max(1);
    let (commands, command_consumer) = RingBuffer::new(command_capacity);
    Ok((
        PlaybackVoiceController {
            voices,
            commands,
            reserved: vec![false; usize::from(capacity)],
            command_capacity,
            command_queue_full: 0,
            destination_channels,
        },
        PlaybackVoiceRenderer {
            voices: realtime_voices,
            commands: command_consumer,
            retained_activation: None,
            destination_channels: usize::from(destination_channels),
            maximum_block_frames,
            completed_voices: 0,
            stopped_voices: 0,
            failed_voices: 0,
        },
    ))
}

fn event_from_retired(retired: RetiredVoice<PlaybackVoice>) -> PlaybackRenderEvent {
    let RetiredVoice { id, reason, payload } = retired;
    let PlaybackVoice {
        playback_id,
        path,
        source,
        rendered_frames,
        stop_reason,
        ..
    } = *payload;
    let stream_failed = matches!(&source, PlaybackVoiceSource::Stream { reader, .. } if reader.state().failed);
    if stream_failed {
        PlaybackRenderEvent::Failed {
            playback_id,
            path,
            voice: id,
            rendered_frames,
        }
    } else if reason == VoiceRetirementReason::Completed {
        PlaybackRenderEvent::Finished {
            playback_id,
            path,
            voice: id,
            rendered_frames,
        }
    } else {
        PlaybackRenderEvent::Stopped {
            playback_id,
            path,
            voice: id,
            reason: stop_reason.unwrap_or(PlaybackStopReason::Requested),
            rendered_frames,
        }
    }
}
