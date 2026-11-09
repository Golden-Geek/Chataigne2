use super::*;

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

fn policy(enabled: bool) -> VideoTransportSettings {
    VideoTransportSettings {
        play_at_load: enabled,
        ..Default::default()
    }
}
fn playing(control: &VideoControl) -> bool {
    control.sample().unwrap().playing
}

#[test]
fn default_load_waits_paused_and_enabled_load_waits_for_readiness() {
    for enabled in [false, true] {
        let control = VideoControl::default();
        control.initialize(policy(enabled)).unwrap();
        assert!(!playing(&control));
        control.prepare(seconds(60)).unwrap();
        assert_eq!(playing(&control), enabled);
    }
}

#[test]
fn pause_and_stop_cancel_pending_load_and_explicit_play_wins_policy_cancellation() {
    for mode in [VideoPlaybackMode::Paused, VideoPlaybackMode::Stopped] {
        let control = VideoControl::default();
        control.initialize(policy(true)).unwrap();
        control.command(mode).unwrap();
        control.prepare(seconds(60)).unwrap();
        assert!(!playing(&control));
        assert_eq!(control.settings().unwrap().mode, mode);
    }
    let control = VideoControl::default();
    control.initialize(policy(true)).unwrap();
    control.command(VideoPlaybackMode::Playing).unwrap();
    let mut settings = control.settings().unwrap();
    settings.play_at_load = false;
    control.configure(settings, false).unwrap();
    control.prepare(seconds(60)).unwrap();
    assert!(playing(&control));
}

#[test]
fn pending_policy_changes_apply_before_ready_but_ready_changes_wait_for_next_load() {
    let control = VideoControl::default();
    control.initialize(policy(false)).unwrap();
    control.configure(policy(true), false).unwrap();
    let mut cancel = control.settings().unwrap();
    cancel.play_at_load = false;
    control.configure(cancel, false).unwrap();
    control.prepare(seconds(60)).unwrap();
    assert!(!playing(&control));
    let mut next = control.settings().unwrap();
    next.play_at_load = true;
    control.configure(next.clone(), false).unwrap();
    assert!(
        !playing(&control),
        "ready media must not start just because the load policy changes"
    );
    control.initialize(next).unwrap();
    assert!(playing(&control));
}

#[test]
fn repeat_play_restarts_after_end_and_retired_media_rejects_commands() {
    let control = VideoControl::default();
    control.prepare(seconds(60)).unwrap();
    control.command(VideoPlaybackMode::Playing).unwrap();
    control.0.lock().unwrap().transport.seek(seconds(60)).unwrap();
    assert!(!playing(&control));
    control.command(VideoPlaybackMode::Playing).unwrap();
    assert!(playing(&control));
    control.retire_media();
    assert!(control.command(VideoPlaybackMode::Playing).is_err());
}
