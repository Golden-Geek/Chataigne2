use super::*;
use std::time::{Duration, Instant};

fn preparer() -> TelemetryPreparer {
    TelemetryPreparer {
        pool: PayloadPool::new("telemetry-test", 3, 2 * SCRATCH_BYTES).unwrap(),
        voices: Voices::default(),
        previous: None,
    }
}

#[test]
fn small_telemetry_frames_admit_below_the_maximum_scratch_allowance() {
    let pool = PayloadPool::new("small-telemetry-test", 3, 32 * 1024 * 1024).unwrap();
    let mut preparer = TelemetryPreparer {
        pool: pool.clone(),
        voices: Voices::default(),
        previous: None,
    };
    let snapshot = AudioObservationSnapshot::default();
    let first = preparer.prepare(&snapshot).unwrap();
    let second = preparer.prepare(&snapshot).unwrap();
    assert!(first.same_payload(&second));
    drop((first, second, preparer));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn native_telemetry_preserves_wire_shape_shares_unchanged_frames_and_exported_quota() {
    let mut preparer = preparer();
    let pool = preparer.pool.clone();
    let mut snapshot = AudioObservationSnapshot::default();
    let first = preparer.prepare(&snapshot).unwrap();
    let unchanged = preparer.prepare(&snapshot).unwrap();
    assert!(first.same_payload(&unchanged));
    drop(unchanged);
    let event = first.event(NodeId(1));
    let decoded: chataigne_sound_card_protocol::SoundCardUiTelemetryDto =
        serde_json::from_value(event.payload.as_ref().clone()).unwrap();
    assert_eq!(decoded, super::super::telemetry_from_observation(&snapshot, vec![]));
    let exported = golden_core::ui_sync::UiEventDto::from(golden_core::events::Event::custom(
        golden_core::engine::EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
        event,
    ));
    snapshot.render_frame = 1;
    let second = preparer.prepare(&snapshot).unwrap();
    assert!(!first.same_payload(&second));
    assert_eq!(
        preparer.prepare(&snapshot).unwrap_err(),
        ObservationCaptureError::TelemetryCapacity
    );
    drop(first);
    assert!(
        preparer.prepare(&snapshot).is_err(),
        "exported DTO must pin its canonical reservation"
    );
    drop(exported);
    let deadline = Instant::now() + Duration::from_secs(3);
    while preparer.prepare(&snapshot).is_err() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    drop((preparer, second));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn telemetry_rejects_oversized_wire_and_value_strings_before_admission() {
    let mut preparer = preparer();
    let mut snapshot = AudioObservationSnapshot::default();
    snapshot.device.input.selected_label = Some("x".repeat(WIRE_BYTES + 1));
    assert_eq!(
        preparer.prepare(&snapshot).unwrap_err(),
        ObservationCaptureError::TelemetryOversized
    );
    snapshot.device.input.selected_label = None;
    snapshot.analysis.taps.push(golden_audio::AnalysisTapObservation {
        tap: golden_audio::AnalysisTapId::new(),
        source: golden_audio::AudioChannelId::new(),
        enabled: true,
        result: Some(golden_audio::AnalysisResult::Pitch(golden_audio::PitchObservation {
            note_name: "x".repeat(65),
            ..golden_audio::PitchObservation::invalid()
        })),
    });
    assert_eq!(
        preparer.prepare(&snapshot).unwrap_err(),
        ObservationCaptureError::ValueOversized
    );
    assert!(preparer.pool.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn native_voice_projection_preserves_replacements_and_failed_loads_with_sorted_identity() {
    use golden_audio::{AudioError, PlaybackId, PlaybackInfo, PlaybackStopInfo, PlaybackStopReason, VoiceId};
    let first = PlaybackInfo {
        playback_id: PlaybackId::new("voice-b").unwrap(),
        path: "first.wav".into(),
        voice: VoiceId::new(0, 1),
    };
    let replacement = PlaybackInfo {
        path: "second.wav".into(),
        voice: VoiceId::new(0, 2),
        ..first.clone()
    };
    let mut voices = Voices::default();
    voices.observe(&AudioEvent::PlaybackStarted(first.clone()));
    voices.observe(&AudioEvent::PlaybackStarted(replacement.clone()));
    voices.observe(&AudioEvent::PlaybackStopped(PlaybackStopInfo {
        playback_id: first.playback_id.clone(),
        voice: Some(first.voice),
        reason: PlaybackStopReason::Replaced,
    }));
    voices.observe(&AudioEvent::PlaybackFinished(first));
    voices.observe(&AudioEvent::PlaybackFailed(golden_audio::PlaybackFailure {
        terminated_voice: None,
        playback_id: replacement.playback_id.clone(),
        path: "missing.wav".into(),
        error: AudioError::invalid_configuration("missing file"),
    }));
    let mut earlier = replacement.clone();
    earlier.playback_id = PlaybackId::new("voice-a").unwrap();
    voices.observe(&AudioEvent::PlaybackStarted(earlier));
    let projected = voices.project(WIRE_BYTES).unwrap();
    assert_eq!(
        projected
            .iter()
            .map(|voice| voice.playback_id.as_str())
            .collect::<Vec<_>>(),
        ["voice-a", "voice-b"]
    );
    assert_eq!(projected[1].path, "second.wav");
    assert_eq!(projected[1].voice, replacement.voice.to_string());
    voices.observe(&AudioEvent::PlaybackFailed(golden_audio::PlaybackFailure {
        terminated_voice: Some(replacement.voice),
        playback_id: replacement.playback_id,
        path: replacement.path,
        error: AudioError::invalid_configuration("decoder failed"),
    }));
    assert_eq!(voices.project(WIRE_BYTES).unwrap().len(), 1);
    voices.observe(&AudioEvent::ShutdownComplete);
    assert!(voices.project(WIRE_BYTES).unwrap().is_empty());
}

#[test]
fn native_voice_contract_rejection_and_utf8_projection_bounds_are_visible() {
    use golden_audio::{PlaybackId, PlaybackInfo, VoiceId};
    let mut voices = Voices::default();
    for index in 0..=super::super::super::MAX_PLAYBACK_VOICES {
        voices.observe(&AudioEvent::PlaybackStarted(PlaybackInfo {
            playback_id: PlaybackId::new(index.to_string()).unwrap(),
            path: "file.wav".into(),
            voice: VoiceId::new(index, 1),
        }));
    }
    assert_eq!(
        voices.project(WIRE_BYTES).unwrap_err(),
        ObservationCaptureError::VoiceCapacity
    );
    voices.observe(&AudioEvent::ShutdownComplete);
    voices.observe(&AudioEvent::PlaybackStarted(PlaybackInfo {
        playback_id: PlaybackId::new("x".repeat(4096)).unwrap(),
        path: "file.wav".into(),
        voice: VoiceId::new(0, 1),
    }));
    assert_eq!(
        voices.project(1024).unwrap_err(),
        ObservationCaptureError::TelemetryOversized
    );
    assert!(voices.project(WIRE_BYTES).is_ok());
}
