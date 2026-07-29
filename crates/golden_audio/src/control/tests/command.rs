use std::time::Duration;

use crate::{GainDb, PlayFileRequest, PlaybackId};

#[test]
fn play_file_request_defaults_preserve_replacement_from_the_start() {
    let request = PlayFileRequest::new("fixture.wav", PlaybackId::new("voice").unwrap());

    assert_eq!(request.gain, GainDb::UNITY);
    assert_eq!(request.start_offset, Duration::ZERO);
    assert!(request.force_restart);
}

#[test]
fn play_file_request_builders_set_offset_restart_policy_and_gain() {
    let gain = GainDb::new(-6.0).unwrap();
    let request = PlayFileRequest::new("fixture.wav", PlaybackId::new("voice").unwrap())
        .with_gain(gain)
        .with_start_offset(Duration::from_millis(375))
        .with_force_restart(false);

    assert_eq!(request.gain, gain);
    assert_eq!(request.start_offset, Duration::from_millis(375));
    assert!(!request.force_restart);
}

#[test]
fn missing_serialized_playback_options_use_constructor_defaults() {
    let mut value =
        serde_json::to_value(PlayFileRequest::new("fixture.wav", PlaybackId::new("voice").unwrap())).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("start_offset");
    object.remove("force_restart");

    let request: PlayFileRequest = serde_json::from_value(value).unwrap();

    assert_eq!(request.start_offset, Duration::ZERO);
    assert!(request.force_restart);
}
