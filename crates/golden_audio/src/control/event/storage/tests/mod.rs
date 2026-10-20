use super::*;
use crate::{
    AudioDeviceId, AudioDeviceProfileKey, AudioDirection, AudioInspectorError, AudioStreamStatus, BackendId,
    DiagnosticEvent, DiagnosticSeverity, PlaybackFailure, PlaybackId,
};

#[test]
fn diagnostic_preparation_bounds_context_and_text_and_marks_truncation() {
    let mut diagnostic = DiagnosticEvent::new(DiagnosticSeverity::Warning, "c".repeat(10_000), "é".repeat(10_000));
    for index in 0..64 {
        diagnostic
            .context
            .insert(format!("{index:03}{}", "k".repeat(1000)), "v".repeat(8000));
    }
    let mut event = AudioEvent::Diagnostic(diagnostic);
    event.prepare_delivery();
    assert!(event.retained_bytes() < crate::control::delivery::EVENT_PAYLOAD_BYTES);
    let AudioEvent::Diagnostic(diagnostic) = event else {
        panic!("diagnostic")
    };
    assert!(diagnostic.code.len() <= KEY_BYTES);
    assert!(diagnostic.message.len() <= TEXT_BYTES);
    assert!(diagnostic.message.ends_with(TRUNCATED));
    assert!(diagnostic.context.len() <= CONTEXT_ITEMS + 1);
    assert_eq!(diagnostic.context["truncated"], "true");
}

#[test]
fn spare_text_capacity_is_charged_then_compacted_before_publication() {
    let mut message = String::with_capacity(2 * 1024 * 1024);
    message.push_str("small");
    let mut event = AudioEvent::ConfigurationRejected {
        generation: crate::ConfigGeneration::INITIAL,
        error: AudioError::invalid_configuration(message),
    };
    assert!(event.retained_bytes() >= 2 * 1024 * 1024);
    event.prepare_delivery();
    assert!(event.retained_bytes() < 4096);
    let AudioEvent::ConfigurationRejected { error, .. } = event else {
        panic!("error")
    };
    assert_eq!(error.message, "small");
}

#[test]
fn largest_stream_identities_fit_the_event_bound_without_changing_their_meaning() {
    let target = AudioDeviceTargetId::Device {
        backend: BackendId::new("b".repeat(BackendId::MAX_BYTES)).unwrap(),
        device: AudioDeviceId::new("d".repeat(AudioDeviceId::MAX_BYTES)).unwrap(),
    };
    let mut status = AudioStreamStatus::disabled(AudioDirection::Output);
    status.selected_target = Some(target.clone());
    status.active_target = Some(target.clone());
    status.profile_key = Some(AudioDeviceProfileKey::new("p".repeat(AudioDeviceProfileKey::MAX_BYTES)).unwrap());
    status.selected_label = Some("label".repeat(10_000));
    status.error = Some(AudioInspectorError {
        category: crate::AudioErrorCategory::DeviceMissing,
        message: "m".repeat(50_000),
        technical_detail: Some("t".repeat(50_000)),
    });
    let mut event = AudioEvent::DeviceStatusChanged(status);
    event.prepare_delivery();
    assert!(event.retained_bytes() < crate::control::delivery::EVENT_PAYLOAD_BYTES);
    let AudioEvent::DeviceStatusChanged(status) = event else {
        panic!("status")
    };
    assert_eq!(status.selected_target, Some(target.clone()));
    assert_eq!(status.active_target, Some(target));
    assert_eq!(
        status.profile_key.unwrap().as_str().len(),
        AudioDeviceProfileKey::MAX_BYTES
    );
}

#[test]
fn largest_playback_failure_preserves_accepted_identity_and_path() {
    let path = std::path::PathBuf::from("x".repeat(crate::PlayFileRequest::MAX_PATH_BYTES));
    let id = PlaybackId::new("p".repeat(PlaybackId::MAX_BYTES)).unwrap();
    let mut event = AudioEvent::PlaybackFailed(PlaybackFailure {
        terminated_voice: None,
        playback_id: id.clone(),
        path: path.clone(),
        error: AudioError::invalid_configuration("failure".repeat(50_000)),
    });
    event.prepare_delivery();
    assert!(event.retained_bytes() < crate::control::delivery::EVENT_PAYLOAD_BYTES);
    let AudioEvent::PlaybackFailed(failure) = event else {
        panic!("failure")
    };
    assert_eq!(failure.playback_id, id);
    assert_eq!(failure.path, path);
    assert!(failure.error.message.ends_with(TRUNCATED));
}

#[test]
fn identifier_bounds_apply_to_construction_deserialization_and_spare_capacity() {
    let mut source = String::with_capacity(1024 * 1024);
    source.push_str("identity");
    let id = PlaybackId::new(source).unwrap();
    assert_eq!(id.retained_heap_bytes(), "identity".len());
    assert!(PlaybackId::new("a".repeat(PlaybackId::MAX_BYTES + 1)).is_err());
    for invalid in [
        "".to_owned(),
        " spaced ".to_owned(),
        "a".repeat(PlaybackId::MAX_BYTES + 1),
    ] {
        let encoded = serde_json::to_string(&invalid).unwrap();
        assert!(serde_json::from_str::<PlaybackId>(&encoded).is_err());
        assert!(serde_json::from_str::<BackendId>(&encoded).is_err());
        assert!(serde_json::from_str::<AudioDeviceId>(&encoded).is_err());
        assert!(serde_json::from_str::<AudioDeviceProfileKey>(&encoded).is_err());
        assert!(serde_json::from_str::<crate::PhysicalChannelKey>(&encoded).is_err());
    }
    assert_eq!(serde_json::from_str::<PlaybackId>("\"identity\"").unwrap(), id);
}
