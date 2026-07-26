use chataigne_sound_card_protocol::{
    SoundCardPlaybackLifecycle, SoundCardPlaybackVoiceDto,
};
use golden_audio::AudioEvent;

use super::SoundCardRuntime;

impl SoundCardRuntime {
    pub(super) fn observe_event(&mut self, event: &AudioEvent) {
        match event {
            AudioEvent::ConfigurationRejected { error, .. } => {
                self.last_error = Some(error.to_string());
            }
            AudioEvent::ConfigurationApplied { .. } => {
                self.last_error = None;
            }
            AudioEvent::PlaybackFailed(failure) => {
                self.active_playback_voices.remove(&failure.playback_id);
                self.last_error = Some(failure.error.to_string());
            }
            AudioEvent::PlaybackStarted(info) => {
                self.active_playback_voices.insert(
                    info.playback_id.clone(),
                    SoundCardPlaybackVoiceDto {
                        playback_id: info.playback_id.to_string(),
                        path: info.path.to_string_lossy().into_owned(),
                        voice: info.voice.to_string(),
                        lifecycle: SoundCardPlaybackLifecycle::Playing,
                    },
                );
            }
            AudioEvent::PlaybackFinished(info) => {
                self.active_playback_voices.remove(&info.playback_id);
            }
            AudioEvent::PlaybackStopped(info) => {
                self.active_playback_voices.remove(&info.playback_id);
            }
            AudioEvent::Diagnostic(diagnostic) => {
                self.last_error = Some(diagnostic.message.clone());
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn observe_event_for_test(&mut self, event: &AudioEvent) {
        self.observe_event(event);
    }

    #[cfg(test)]
    pub(crate) fn playback_voices_for_test(&self) -> Vec<SoundCardPlaybackVoiceDto> {
        let mut voices = self
            .active_playback_voices
            .values()
            .cloned()
            .collect::<Vec<_>>();
        voices.sort_by(|left, right| left.playback_id.cmp(&right.playback_id));
        voices
    }
}
