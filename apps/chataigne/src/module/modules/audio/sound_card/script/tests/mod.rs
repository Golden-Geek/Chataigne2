use std::{path::PathBuf, time::Duration};

use golden_core::parameter::ParamValue;

use super::script_request;
use crate::app::module_modules_audio_sound_card_commands::SoundCardCommandRequest;

#[test]
fn play_file_script_keeps_legacy_defaults() {
    let request = play_file_request(&[
        ParamValue::Str("clip.wav".to_string()),
        ParamValue::Str("music".to_string()),
    ])
    .expect("legacy two-argument playFile call");

    assert_eq!(
        request,
        SoundCardCommandRequest::PlayFile {
            path: PathBuf::from("clip.wav"),
            playback_id: golden_audio::PlaybackId::new("music").expect("playback ID"),
            start_offset: Duration::ZERO,
            force_restart: true,
        }
    );
}

#[test]
fn play_file_script_accepts_optional_offset_and_restart() {
    let offset_only = play_file_request(&[
        ParamValue::Str("clip.wav".to_string()),
        ParamValue::Str("music".to_string()),
        ParamValue::Float(2.5),
    ])
    .expect("three-argument playFile call");
    assert!(matches!(
        offset_only,
        SoundCardCommandRequest::PlayFile {
            start_offset,
            force_restart: true,
            ..
        } if start_offset == Duration::from_millis(2_500)
    ));

    let extended = play_file_request(&[
        ParamValue::Str("clip.wav".to_string()),
        ParamValue::Str("music".to_string()),
        ParamValue::Float(1.25),
        ParamValue::Bool(false),
    ])
    .expect("four-argument playFile call");
    assert!(matches!(
        extended,
        SoundCardCommandRequest::PlayFile {
            start_offset,
            force_restart: false,
            ..
        } if start_offset == Duration::from_millis(1_250)
    ));
}

#[test]
fn play_file_script_rejects_invalid_optional_arguments() {
    let invalid_offset = play_file_request(&[
        ParamValue::Str("clip.wav".to_string()),
        ParamValue::Str("music".to_string()),
        ParamValue::Float(-0.25),
    ])
    .expect_err("negative playback offset");
    assert!(invalid_offset.contains("startOffsetSeconds"));

    let invalid_restart = play_file_request(&[
        ParamValue::Str("clip.wav".to_string()),
        ParamValue::Str("music".to_string()),
        ParamValue::Float(0.0),
        ParamValue::Str("yes".to_string()),
    ])
    .expect_err("non-boolean forceRestart");
    assert!(invalid_restart.contains("forceRestart"));

    let too_many = play_file_request(&[
        ParamValue::Str("clip.wav".to_string()),
        ParamValue::Str("music".to_string()),
        ParamValue::Float(0.0),
        ParamValue::Bool(true),
        ParamValue::Bool(true),
    ])
    .expect_err("five-argument playFile call");
    assert!(too_many.contains("expects 2 to 4"));
}

fn play_file_request(args: &[ParamValue]) -> Result<SoundCardCommandRequest, String> {
    script_request("playFile", args).expect("playFile should be a registered Sound Card script method")
}
