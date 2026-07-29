use std::path::PathBuf;

use golden_audio::{
    AudioError, AudioEvent, CommandSequence, PlaybackCommandIgnored, PlaybackFailure, PlaybackId,
    PlaybackInfo, PlaybackStopInfo, PlaybackStopReason, VoiceId,
};

use super::super::traffic_logging::outgoing_audio_log_message;

#[test]
fn playback_lifecycle_events_have_outgoing_log_messages() {
    let playback_id = PlaybackId::new("voice-a").expect("playback ID");
    let started = AudioEvent::PlaybackStarted(PlaybackInfo {
        playback_id: playback_id.clone(),
        path: PathBuf::from("music/intro.wav"),
        voice: VoiceId::new(2, 4),
    });
    assert_eq!(
        outgoing_audio_log_message(&started).as_deref(),
        Some("Started playing audio file `music/intro.wav` with playback ID `voice-a`.")
    );

    let finished = AudioEvent::PlaybackFinished(PlaybackInfo {
        playback_id: playback_id.clone(),
        path: PathBuf::from("music/intro.wav"),
        voice: VoiceId::new(2, 4),
    });
    assert_eq!(
        outgoing_audio_log_message(&finished).as_deref(),
        Some("Finished playing audio file `music/intro.wav` with playback ID `voice-a`.")
    );

    let stopped = AudioEvent::PlaybackStopped(PlaybackStopInfo {
        playback_id,
        voice: Some(VoiceId::new(2, 4)),
        reason: PlaybackStopReason::Requested,
    });
    assert_eq!(
        outgoing_audio_log_message(&stopped).as_deref(),
        Some("Stopped audio playback `voice-a` (reason: requested).")
    );

    let failed = AudioEvent::PlaybackFailed(PlaybackFailure {
        playback_id: PlaybackId::new("voice-b").expect("playback ID"),
        path: PathBuf::from("music/missing.wav"),
        error: AudioError::invalid_configuration("fixture failure"),
    });
    let failure_message = outgoing_audio_log_message(&failed).expect("playback failure log");
    assert!(failure_message.contains("Audio playback `voice-b`"));
    assert!(failure_message.contains("fixture failure"));
}

#[test]
fn ignored_playback_commands_explain_why_they_had_no_effect() {
    let ignored_play = AudioEvent::PlaybackCommandIgnored(
        PlaybackCommandIgnored::PlayFileAlreadyActive {
        sequence: CommandSequence::new(7).expect("sequence"),
        playback_id: PlaybackId::new("music").expect("playback ID"),
        path: PathBuf::from("music/loop.wav"),
    });
    assert!(
        outgoing_audio_log_message(&ignored_play)
            .expect("ignored play log")
            .contains("Force Restart is disabled")
    );

    let ignored_stop = AudioEvent::PlaybackCommandIgnored(
        PlaybackCommandIgnored::StopFileNotFound {
            sequence: CommandSequence::new(8).expect("sequence"),
            playback_id: PlaybackId::new("missing").expect("playback ID"),
        },
    );
    assert_eq!(
        outgoing_audio_log_message(&ignored_stop).as_deref(),
        Some(
            "Stop audio playback `missing` had no effect because no active or loading playback uses that ID."
        )
    );

    let ignored_stop_all = AudioEvent::PlaybackCommandIgnored(
        PlaybackCommandIgnored::StopAllFilesEmpty {
            sequence: CommandSequence::new(9).expect("sequence"),
        },
    );
    assert_eq!(
        outgoing_audio_log_message(&ignored_stop_all).as_deref(),
        Some(
            "Stop all audio files had no effect because no audio files are active or loading."
        )
    );
}
