use std::collections::BTreeMap;

use chataigne_sound_card_protocol::{SoundCardPlaybackLifecycle, SoundCardPlaybackVoiceDto};
use golden_audio::{AudioEvent, PlaybackId, PlaybackInfo};

use super::ObservationCaptureError;

/// Matches the app's explicit native voice admission limit. Oversized future
/// native contracts reject telemetry; ordered lifecycle callbacks are unaffected.
use super::super::super::MAX_PLAYBACK_VOICES as MAX_VOICES;
const MAX_BYTES: usize = 128 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Voices {
    active: BTreeMap<PlaybackId, PlaybackInfo>,
    bytes: usize,
    rejected: bool,
}

fn weight(info: &PlaybackInfo) -> usize {
    1024 + 2 * info.playback_id.retained_heap_bytes() + info.path.capacity()
}

impl Voices {
    pub(super) fn observe(&mut self, event: &AudioEvent) {
        match event {
            AudioEvent::PlaybackStarted(info) => {
                let previous = self.active.get(&info.playback_id).map_or(0, weight);
                let next = self.bytes - previous + weight(info);
                if next > MAX_BYTES || (previous == 0 && self.active.len() >= usize::from(MAX_VOICES)) {
                    self.rejected = true;
                    return;
                }
                self.active.insert(info.playback_id.clone(), info.clone());
                self.bytes = next;
            }
            AudioEvent::PlaybackFinished(info) => {
                if self
                    .active
                    .get(&info.playback_id)
                    .is_some_and(|current| current.voice == info.voice)
                {
                    self.remove(&info.playback_id);
                }
            }
            AudioEvent::PlaybackStopped(info) => {
                if self
                    .active
                    .get(&info.playback_id)
                    .is_some_and(|current| info.voice == Some(current.voice))
                {
                    self.remove(&info.playback_id);
                }
            }
            AudioEvent::PlaybackFailed(failure) => {
                if self
                    .active
                    .get(&failure.playback_id)
                    .is_some_and(|current| failure.terminated_voice == Some(current.voice))
                {
                    self.remove(&failure.playback_id);
                }
            }
            // A failed load/control admission has no terminal identity; an
            // already-playing generation remains until its own native outcome.
            AudioEvent::ShutdownComplete => {
                self.active.clear();
                self.bytes = 0;
                self.rejected = false;
            }
            _ => {}
        }
    }

    fn remove(&mut self, id: &PlaybackId) {
        if let Some(info) = self.active.remove(id) {
            self.bytes -= weight(&info);
        }
    }

    pub(super) fn projection_bytes(&self, maximum: usize) -> Result<usize, ObservationCaptureError> {
        if self.rejected {
            return Err(ObservationCaptureError::VoiceCapacity);
        }
        // Bound UTF-8 lossy expansion before building the DTO strings. The
        // encoder independently enforces its exact wire limit afterwards.
        let mut bytes = 0usize;
        for info in self.active.values() {
            bytes = bytes
                .saturating_add(info.playback_id.as_str().len())
                .saturating_add(info.path.as_os_str().len().saturating_mul(3))
                .saturating_add(256);
            if bytes > maximum {
                return Err(ObservationCaptureError::TelemetryOversized);
            }
        }
        Ok(bytes)
    }

    pub(super) fn project(&self, maximum: usize) -> Result<Vec<SoundCardPlaybackVoiceDto>, ObservationCaptureError> {
        self.projection_bytes(maximum)?;
        Ok(self
            .active
            .values()
            .map(|info| SoundCardPlaybackVoiceDto {
                playback_id: info.playback_id.to_string(),
                path: info.path.to_string_lossy().into_owned(),
                voice: info.voice.to_string(),
                lifecycle: SoundCardPlaybackLifecycle::Playing,
            })
            .collect())
    }
}
