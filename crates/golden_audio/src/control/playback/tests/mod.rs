use super::super::delivery::event_channel;
use super::*;
use crate::{AudioEventReceiver, PlaybackSchedulerConfig, VoiceId};

struct Fixture {
    sender: EventSender,
    events: AudioEventReceiver,
    observations: Arc<RwLock<AudioObservationSnapshot>>,
    scheduler: PlaybackScheduler,
    voices: PlaybackVoiceController,
    _renderer: crate::PlaybackVoiceRenderer,
    lifecycle: PlaybackLifecycles,
}
impl Fixture {
    fn new() -> Self {
        let config = AudioEngineConfig::default();
        let limits = EngineLimits::default();
        let (voices, renderer) = crate::playback_voice_pool(4, 1, 64).unwrap();
        let (sender, events) = event_channel(32, true);
        Self {
            sender,
            events,
            observations: Arc::new(RwLock::new(AudioObservationSnapshot::default())),
            scheduler: PlaybackScheduler::new(PlaybackSchedulerConfig::from_engine(&config, &limits)).unwrap(),
            voices,
            _renderer: renderer,
            lifecycle: PlaybackLifecycles::default(),
        }
    }
    fn finish(&mut self) {
        finish_playback_shutdown(
            &self.sender,
            &self.observations,
            &mut self.scheduler,
            &mut self.voices,
            &mut self.lifecycle,
        );
    }
    fn playing(&mut self, name: &str, sequence: u64, voice: VoiceId) {
        let id = PlaybackId::new(name).unwrap();
        self.lifecycle.current.insert(
            id.clone(),
            PlaybackLifecycle::Playing {
                sequence: CommandSequence::new(sequence).unwrap(),
                info: PlaybackInfo {
                    playback_id: id,
                    path: "fixture.wav".into(),
                    voice,
                },
            },
        );
    }
    fn stop(&mut self, id: &PlaybackId, reason: PlaybackStopReason) {
        assert!(stop_playback(
            &self.sender,
            &self.observations,
            &mut self.scheduler,
            &mut self.voices,
            &mut self.lifecycle,
            id,
            reason
        ));
    }
}

#[test]
fn shutdown_resolves_pending_and_playing_in_original_start_order_once() {
    let mut f = Fixture::new();
    f.playing("active", 2, VoiceId::new(0, 1));
    f.lifecycle.current.insert(
        PlaybackId::new("pending").unwrap(),
        PlaybackLifecycle::Pending {
            sequence: CommandSequence::FIRST,
        },
    );
    f.finish();
    let events = f.events.drain();
    assert!(
        matches!(&events[0], AudioEvent::PlaybackStopped(info) if info.playback_id.as_str() == "pending" && info.voice.is_none())
    );
    assert!(
        matches!(&events[1], AudioEvent::PlaybackStopped(info) if info.playback_id.as_str() == "active" && info.voice == Some(VoiceId::new(0, 1)))
    );
    assert_eq!(events.len(), 2);
    assert_eq!(f.observations.read().unwrap().playback.loading_voices, 0);
    assert_eq!(f.observations.read().unwrap().playback.active_voices, 0);
    f.finish();
    assert!(f.events.drain().is_empty());
}

#[test]
fn shutdown_preserves_a_requested_stop_for_an_old_generation_after_replacement() {
    let mut f = Fixture::new();
    let id = PlaybackId::new("same").unwrap();
    f.playing("same", 1, VoiceId::new(0, 1));
    f.stop(&id, PlaybackStopReason::Replaced);
    f.playing("same", 2, VoiceId::new(0, 2));
    f.finish();
    let events = f.events.drain();
    assert_eq!(events.len(), 2);
    assert!(
        matches!(&events[0], AudioEvent::PlaybackStopped(info) if info.voice == Some(VoiceId::new(0, 1)) && info.reason == PlaybackStopReason::Replaced)
    );
    assert!(
        matches!(&events[1], AudioEvent::PlaybackStopped(info) if info.voice == Some(VoiceId::new(0, 2)) && info.reason == PlaybackStopReason::StopAll)
    );
}

#[test]
fn retired_old_generation_does_not_finish_or_duplicate_its_replacement() {
    let mut f = Fixture::new();
    let id = PlaybackId::new("same").unwrap();
    f.playing("same", 1, VoiceId::new(0, 1));
    f.stop(&id, PlaybackStopReason::Requested);
    f.playing("same", 2, VoiceId::new(0, 2));
    publish_render_event(
        &f.sender,
        &f.observations,
        &mut f.scheduler,
        &mut f.lifecycle,
        PlaybackRenderEvent::Stopped {
            playback_id: id,
            voice: VoiceId::new(0, 1),
            path: "fixture.wav".into(),
            rendered_frames: 0,
            reason: PlaybackStopReason::Requested,
        },
    );
    f.finish();
    let events = f.events.drain();
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], AudioEvent::PlaybackStopped(info) if info.voice == Some(VoiceId::new(0, 1))));
    assert!(matches!(&events[1], AudioEvent::PlaybackStopped(info) if info.voice == Some(VoiceId::new(0, 2))));
}

#[test]
fn stop_all_keeps_unreturned_voice_outcomes_for_shutdown() {
    let mut f = Fixture::new();
    f.playing("active", 1, VoiceId::new(0, 1));
    assert!(stop_all_playback(
        &f.sender,
        &f.observations,
        &mut f.scheduler,
        &mut f.voices,
        &mut f.lifecycle,
        PlaybackStopReason::ModuleDisabled
    ));
    f.finish();
    let events = f.events.drain();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], AudioEvent::PlaybackStopped(info) if info.reason == PlaybackStopReason::ModuleDisabled)
    );
}
