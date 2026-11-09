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

#[test]
fn delayed_metadata_feedback_cannot_restore_older_duration_bounds() {
    use crate::{
        app::{AppEngine, AppNode},
        media::protocol::*,
    };
    use golden_core::node::Node;
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut engine).unwrap();
    let result = crate::media::apply_operation(
        &mut engine,
        MEDIA_OPERATION,
        serde_json::to_value(MediaEdit::CreateMedia {
            media_kind: MediaKind::Video,
            label: None,
        })
        .unwrap(),
    )
    .unwrap();
    let MediaEditResult::Created { reference } = serde_json::from_value(result).unwrap() else {
        panic!()
    };
    engine.settle_edit_notifications(16).unwrap();
    let AppNode::Media(node) = engine
        .nodes
        .get(engine.node_id_by_uuid(reference.media.0).unwrap())
        .unwrap()
    else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let state = control.0.lock().unwrap();
    let publisher = state.feedback.clone().unwrap();
    let sequence = state.feedback_sequence + 2;
    drop(state);
    publisher.publish(
        sequence,
        crate::media::video::feedback::Status {
            playing: false,
            length_ms: 200.0,
        },
    );
    publisher.publish(
        sequence - 1,
        crate::media::video::feedback::Status {
            playing: false,
            length_ms: 100.0,
        },
    );
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let position = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.decl_id.0 == "parameters/video_current_position_ms")
        .unwrap()
        .1;
    assert_eq!(
        position.engine_param_snapshot().unwrap().constraints.range,
        golden_core::parameter::RangeConstraint::uniform(Some(0.0), Some(200.0))
    );
}
