use super::*;
use std::sync::Arc;

fn control(on_activate: VideoOnActivate, on_deactivate: VideoOnDeactivate) -> Arc<VideoControl> {
    let control = Arc::new(VideoControl::default());
    control.prepare(Duration::from_secs(10)).unwrap();
    control
        .configure(
            VideoTransportSettings {
                position_ms: 2500.0,
                speed: 0.0,
                on_activate,
                on_deactivate,
                ..Default::default()
            },
            true,
            None,
        )
        .unwrap();
    control
}

#[test]
fn all_matrix_policies_keep_outgoing_transport_until_actual_release() {
    for activation in [
        VideoOnActivate::PlayFromStart,
        VideoOnActivate::Resume,
        VideoOnActivate::Nothing,
    ] {
        for deactivation in [
            VideoOnDeactivate::Stop,
            VideoOnDeactivate::Pause,
            VideoOnDeactivate::Nothing,
        ] {
            let control = control(activation, deactivation);
            let mut lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
            let active = control.sample().unwrap();
            let expected_position = if activation == VideoOnActivate::PlayFromStart {
                Duration::ZERO
            } else {
                Duration::from_millis(2500)
            };
            assert_eq!(active.position, expected_position);
            assert_eq!(active.playing, activation != VideoOnActivate::Nothing);
            lease.deactivate().unwrap();
            lease.deactivate().unwrap();
            let outgoing = control.sample().unwrap();
            assert_eq!(outgoing.epoch, active.epoch, "outgoing starts no transport action");
            assert_eq!(outgoing.playing, active.playing);
            lease.finish().unwrap();
            let final_state = control.sample().unwrap();
            assert_eq!(
                final_state.playing,
                deactivation == VideoOnDeactivate::Nothing && active.playing
            );
            assert_eq!(
                final_state.position,
                if deactivation == VideoOnDeactivate::Stop {
                    Duration::ZERO
                } else {
                    expected_position
                }
            );
            assert_eq!(
                control.settings().unwrap().mode,
                match deactivation {
                    VideoOnDeactivate::Stop => VideoPlaybackMode::Stopped,
                    VideoOnDeactivate::Pause => VideoPlaybackMode::Paused,
                    VideoOnDeactivate::Nothing if active.playing => VideoPlaybackMode::Playing,
                    VideoOnDeactivate::Nothing => VideoPlaybackMode::Paused,
                }
            );
        }
    }
}

#[test]
fn references_share_activation_and_last_outgoing_use_without_duplicate_restart() {
    let control = control(VideoOnActivate::PlayFromStart, VideoOnDeactivate::Stop);
    let identity = uuid::Uuid::new_v4();
    let mut first = control.activate_for_matrix(identity).unwrap();
    let epoch = control.sample().unwrap().epoch;
    let mut reference = control.activate_for_matrix(identity).unwrap();
    assert_eq!(control.sample().unwrap().epoch, epoch);
    first.deactivate().unwrap();
    first.finish().unwrap();
    assert!(control.sample().unwrap().playing, "another consumer stays active");
    reference.deactivate().unwrap();
    assert!(
        control.sample().unwrap().playing,
        "the last outgoing consumer retains the clock"
    );
    reference.finish().unwrap();
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn stale_outgoing_completion_cannot_stop_a_reactivated_or_still_shared_video() {
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Stop);
    let mut old = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    old.deactivate().unwrap();
    let mut new = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    let epoch = control.sample().unwrap().epoch;
    old.finish().unwrap();
    assert!(control.sample().unwrap().playing);
    assert_eq!(control.sample().unwrap().epoch, epoch);
    let shared = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    new.deactivate().unwrap();
    new.finish().unwrap();
    assert!(control.sample().unwrap().playing);
    shared.finish().unwrap();
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn newer_manual_transport_cancels_deferred_deactivation_but_policy_edits_do_not() {
    for manual in [VideoPlaybackMode::Playing, VideoPlaybackMode::Paused] {
        let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Stop);
        let mut lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
        lease.deactivate().unwrap();
        control.command(manual).unwrap();
        let before = control.sample().unwrap();
        lease.finish().unwrap();
        let after = control.sample().unwrap();
        assert_eq!(after.epoch, before.epoch);
        assert_eq!(after.position, Duration::from_millis(2500));
        assert_eq!(control.settings().unwrap().mode, manual);
    }
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Pause);
    let mut lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    lease.deactivate().unwrap();
    let mut settings = control.settings().unwrap();
    settings.on_deactivate = VideoOnDeactivate::Stop;
    control.configure(settings, false, None).unwrap();
    lease.finish().unwrap();
    assert_eq!(
        control.settings().unwrap().mode,
        VideoPlaybackMode::Paused,
        "outgoing uses its captured policy"
    );
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(2500));
}

#[test]
fn manual_seek_after_outgoing_and_retired_project_both_fence_old_release() {
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Stop);
    let mut lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    lease.deactivate().unwrap();
    let mut settings = control.settings().unwrap();
    settings.position_ms = 4000.0;
    control.configure(settings, true, None).unwrap();
    lease.finish().unwrap();
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(4000));
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Playing);
    let mut lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    lease.deactivate().unwrap();
    control.retire_media();
    let before = control.sample().unwrap();
    assert!(control.activate_for_matrix(uuid::Uuid::new_v4()).is_err());
    lease.finish().unwrap();
    assert_eq!(control.sample().unwrap().epoch, before.epoch);
}

#[test]
fn active_drop_is_an_immediate_cut_and_resume_retains_the_paused_position() {
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Pause);
    drop(control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap());
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Paused);
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(2500));
    let lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    assert!(control.sample().unwrap().playing);
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(2500));
    lease.finish().unwrap();
}

#[test]
fn restored_media_has_a_new_use_generation_even_when_old_leases_remain_alive() {
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Stop);
    let identity = uuid::Uuid::new_v4();
    let old = control.activate_for_matrix(identity).unwrap();
    control.retire_media();
    control.activate_media();
    let mut restored = control.activate_for_matrix(identity).unwrap();
    drop(old);
    assert!(
        control.sample().unwrap().playing,
        "old incarnation cannot stop the restored owner"
    );
    restored.deactivate().unwrap();
    restored.finish().unwrap();
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn a_late_reference_to_outgoing_content_preserves_newer_manual_transport() {
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Stop);
    let identity = uuid::Uuid::new_v4();
    let mut outgoing = control.activate_for_matrix(identity).unwrap();
    outgoing.deactivate().unwrap();
    control.command(VideoPlaybackMode::Paused).unwrap();
    let epoch = control.sample().unwrap().epoch;
    let late_reference = control.activate_for_matrix(identity).unwrap();
    outgoing.finish().unwrap();
    late_reference.finish().unwrap();
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Paused);
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(2500));
    assert_eq!(control.sample().unwrap().epoch, epoch);
}

#[test]
fn matrix_zero_start_and_resume_do_not_use_manual_plays_endpoint_rewind() {
    for (policy, speed, position) in [
        (VideoOnActivate::PlayFromStart, -1.0, 2500.0),
        (VideoOnActivate::Resume, 1.0, 10000.0),
        (VideoOnActivate::Resume, -1.0, 0.0),
    ] {
        let control = control(policy, VideoOnDeactivate::Nothing);
        let mut settings = control.settings().unwrap();
        settings.speed = speed;
        settings.position_ms = position;
        control.configure(settings, true, None).unwrap();
        let lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
        assert_eq!(
            control.sample().unwrap().position,
            Duration::from_secs_f64(if policy == VideoOnActivate::PlayFromStart {
                0.0
            } else {
                position / 1000.0
            })
        );
        lease.finish().unwrap();
    }
    let control = Arc::new(VideoControl::default());
    let mut settings = control.settings().unwrap();
    settings.speed = -1.0;
    control.configure(settings, false, None).unwrap();
    let lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    control.prepare(Duration::from_secs(10)).unwrap();
    assert_eq!(
        control.sample().unwrap().position,
        Duration::ZERO,
        "late duration readiness retains the Matrix start rule"
    );
    lease.finish().unwrap();
}

#[test]
fn playing_seek_retains_eof_while_explicit_play_still_rewinds() {
    let control = control(VideoOnActivate::Resume, VideoOnDeactivate::Nothing);
    let mut settings = control.settings().unwrap();
    settings.speed = 1.0;
    control.configure(settings, false, None).unwrap();
    let lease = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    let mut settings = control.settings().unwrap();
    settings.position_ms = 10000.0;
    control.configure(settings, true, None).unwrap();
    assert_eq!(control.sample().unwrap().position, Duration::from_secs(10));
    control.command(VideoPlaybackMode::Playing).unwrap();
    assert!(control.sample().unwrap().position < Duration::from_secs(1));
    lease.finish().unwrap();
}
