use super::*;
use crate::media::video::{VideoControl, protocol::*};
use std::{sync::Arc, time::Duration};

fn clock(engine: &AppEngine, media: MediaId) -> Arc<VideoControl> {
    let AppNode::Media(node) = engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()).unwrap() else {
        panic!()
    };
    node.video_control().unwrap()
}
fn tick(engine: &mut AppEngine) {
    engine.run_tick(Duration::from_millis(1)).unwrap();
}
fn policy(engine: &mut AppEngine, media: MediaId, edit: VideoEdit) {
    apply(engine, MediaEdit::Video { media, edit });
}
fn assign(engine: &mut AppEngine, matrix: MediaId, row: LayerId, column: ColumnId, source: MediaId) {
    edit(
        engine,
        matrix,
        MatrixEdit::DropSource {
            layer: row,
            column,
            source: source.0,
            copy: false,
        },
    );
}
fn cut(engine: &mut AppEngine, matrix: MediaId, column: ColumnId) {
    edit(
        engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    tick(engine);
}

#[test]
fn committed_cut_starts_video_without_a_matrix_query_or_renderer_and_unload_stops_it() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    policy(&mut engine, video.media, VideoEdit::Speed { speed: 0.0 });
    policy(&mut engine, video.media, VideoEdit::Seek { position_ms: 2500.0 });
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, video.media);
    let history = engine.undo_len();
    cut(&mut engine, matrix.media, column);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Playing);
    assert_eq!(control.sample().unwrap().position, Duration::ZERO);
    assert!(control.sample().unwrap().playing);
    assert_eq!(engine.undo_len(), history, "runtime playback is not authored history");
    policy(&mut engine, video.media, VideoEdit::Seek { position_ms: 1250.0 });
    cut(&mut engine, matrix.media, column);
    assert_eq!(
        control.sample().unwrap().position,
        Duration::ZERO,
        "same-column reactivation restarts once"
    );
    assert!(control.sample().unwrap().playing);
    edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    tick(&mut engine);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
    assert_eq!(control.sample().unwrap().position, Duration::ZERO);
}

#[test]
fn matrix_cut_replacement_obeys_all_transport_policy_pairs() {
    for on_activate in [
        VideoOnActivate::PlayFromStart,
        VideoOnActivate::Resume,
        VideoOnActivate::Nothing,
    ] {
        for on_deactivate in [
            VideoOnDeactivate::Stop,
            VideoOnDeactivate::Pause,
            VideoOnDeactivate::Nothing,
        ] {
            let mut engine = fixture();
            let video = create(&mut engine, MediaKind::Video);
            let control = clock(&engine, video.media);
            control.prepare(Duration::from_secs(10)).unwrap();
            policy(&mut engine, video.media, VideoEdit::Speed { speed: 0.0 });
            policy(&mut engine, video.media, VideoEdit::Seek { position_ms: 2500.0 });
            policy(&mut engine, video.media, VideoEdit::OnActivate { policy: on_activate });
            policy(
                &mut engine,
                video.media,
                VideoEdit::OnDeactivate { policy: on_deactivate },
            );
            let matrix = create(&mut engine, MediaKind::Matrix);
            let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
            let columns = columns(&mut engine, matrix.media, 2);
            assign(&mut engine, matrix.media, row, columns[0], video.media);
            cut(&mut engine, matrix.media, columns[0]);
            let expected = if on_activate == VideoOnActivate::PlayFromStart {
                Duration::ZERO
            } else {
                Duration::from_millis(2500)
            };
            assert_eq!(control.sample().unwrap().position, expected);
            assert_eq!(
                control.sample().unwrap().playing,
                on_activate != VideoOnActivate::Nothing
            );
            cut(&mut engine, matrix.media, columns[1]);
            assert_eq!(
                control.sample().unwrap().position,
                if on_deactivate == VideoOnDeactivate::Stop {
                    Duration::ZERO
                } else {
                    expected
                }
            );
            assert_eq!(
                control.sample().unwrap().playing,
                on_deactivate == VideoOnDeactivate::Nothing && on_activate != VideoOnActivate::Nothing
            );
        }
    }
}

#[test]
fn nested_references_and_two_matrices_share_one_video_and_cannot_stop_each_other() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    policy(&mut engine, video.media, VideoEdit::Speed { speed: 0.0 });
    policy(
        &mut engine,
        video.media,
        VideoEdit::OnActivate {
            policy: VideoOnActivate::Resume,
        },
    );
    policy(&mut engine, video.media, VideoEdit::Seek { position_ms: 2500.0 });
    let MediaEditResult::ReferenceCreated { media: reference } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(video.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let mut matrices = Vec::new();
    for _ in 0..2 {
        let matrix = create(&mut engine, MediaKind::Matrix);
        let rows = layers(&mut engine, matrix.media, None, 2);
        let column = columns(&mut engine, matrix.media, 1)[0];
        for row in rows {
            assign(&mut engine, matrix.media, layer(row), column, reference);
        }
        cut(&mut engine, matrix.media, column);
        assert!(control.sample().unwrap().playing);
        matrices.push(matrix.media);
    }
    edit(&mut engine, matrices[0], MatrixEdit::ClearPlayback);
    tick(&mut engine);
    assert!(
        control.sample().unwrap().playing,
        "second Matrix still owns the same transport"
    );
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(2500));
    edit(&mut engine, matrices[1], MatrixEdit::ClearPlayback);
    tick(&mut engine);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn discarded_cut_receipt_has_no_video_effect_and_a_later_committed_cut_still_works() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, video.media);
    super::super::playback::launch(&mut engine, matrix.media, column, LaunchMode::Cut).unwrap();
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Paused);
    engine.edits.pending.clear();
    tick(&mut engine);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Paused);
    cut(&mut engine, matrix.media, column);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Playing);
}

#[test]
fn ordinary_column_cut_and_stop_parameters_drive_the_same_video_boundary() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, video.media);
    super::controls::pulse(&mut engine, column.0, "cut");
    golden_core::application::service_script_app_operations(&mut engine).unwrap();
    tick(&mut engine);
    assert!(control.sample().unwrap().playing);
    super::controls::pulse(&mut engine, column.0, "stop");
    golden_core::application::service_script_app_operations(&mut engine).unwrap();
    tick(&mut engine);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn a_cut_video_remains_playing_until_its_actual_outgoing_matrix_frame_finishes() {
    use super::cell_playback::{at, capture, time};
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    assign(&mut engine, matrix.media, row, columns[0], video.media);
    cut(&mut engine, matrix.media, columns[0]);
    let origin = time();
    capture(&engine, matrix.media, row, at(origin, 1000));
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    tick(&mut engine);
    capture(&engine, matrix.media, row, at(origin, 1200));
    assert!(
        control.sample().unwrap().playing,
        "outgoing video keeps its clock during the actual fade"
    );
    capture(&engine, matrix.media, row, at(origin, 1600));
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

fn configured(engine: &mut AppEngine, matrix: MediaId, column: ColumnId, duration_ms: u32) {
    edit(
        engine,
        matrix,
        MatrixEdit::ConfigureColumn {
            column,
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms,
                    curve: TransitionCurve::Linear,
                }),
            },
        },
    );
    edit(
        engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Configured,
        },
    );
    tick(engine);
}
fn finish_without_capture(engine: &mut AppEngine, control: &VideoControl, mode: VideoPlaybackMode) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while control.settings().unwrap().mode != mode && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
        tick(engine);
    }
    assert_eq!(
        control.settings().unwrap().mode,
        mode,
        "deadline wake must finish without any Matrix query or capture"
    );
}

#[test]
fn configured_launch_and_headless_completion_obey_all_nine_policies() {
    for on_activate in [
        VideoOnActivate::PlayFromStart,
        VideoOnActivate::Resume,
        VideoOnActivate::Nothing,
    ] {
        for on_deactivate in [
            VideoOnDeactivate::Stop,
            VideoOnDeactivate::Pause,
            VideoOnDeactivate::Nothing,
        ] {
            let mut engine = fixture();
            let video = create(&mut engine, MediaKind::Video);
            let control = clock(&engine, video.media);
            control.prepare(Duration::from_secs(10)).unwrap();
            for edit in [
                VideoEdit::Speed { speed: 0.0 },
                VideoEdit::Seek { position_ms: 2500.0 },
                VideoEdit::OnActivate { policy: on_activate },
                VideoEdit::OnDeactivate { policy: on_deactivate },
            ] {
                policy(&mut engine, video.media, edit);
            }
            let matrix = create(&mut engine, MediaKind::Matrix);
            let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
            let columns = columns(&mut engine, matrix.media, 2);
            assign(&mut engine, matrix.media, row, columns[0], video.media);
            configured(&mut engine, matrix.media, columns[0], 40);
            assert_eq!(
                control.sample().unwrap().playing,
                on_activate != VideoOnActivate::Nothing
            );
            let expected = if on_activate == VideoOnActivate::PlayFromStart {
                Duration::ZERO
            } else {
                Duration::from_millis(2500)
            };
            assert_eq!(control.sample().unwrap().position, expected);
            configured(&mut engine, matrix.media, columns[1], 40);
            assert_eq!(
                control.sample().unwrap().playing,
                on_activate != VideoOnActivate::Nothing
            );
            // Sleep once beyond the interval; only the coalesced node wake services completion.
            std::thread::sleep(Duration::from_millis(60));
            tick(&mut engine);
            assert_eq!(
                control.sample().unwrap().position,
                if on_deactivate == VideoOnDeactivate::Stop {
                    Duration::ZERO
                } else {
                    expected
                }
            );
            assert_eq!(
                control.sample().unwrap().playing,
                on_deactivate == VideoOnDeactivate::Nothing && on_activate != VideoOnActivate::Nothing
            );
        }
    }
}

#[test]
fn a_captured_fade_finishes_via_deadline_when_rendering_stops() {
    use super::cell_playback::{at, capture, time};
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    assign(&mut engine, matrix.media, row, columns[0], video.media);
    cut(&mut engine, matrix.media, columns[0]);
    let origin = time();
    capture(&engine, matrix.media, row, at(origin, 1000));
    configured(&mut engine, matrix.media, columns[1], 80);
    assert!(control.sample().unwrap().playing);
    finish_without_capture(&mut engine, &control, VideoPlaybackMode::Stopped);
    let result = capture(&engine, matrix.media, row, at(origin, 1300));
    assert_eq!(
        result.inputs.len(),
        0,
        "finished outgoing source leaves the actual frame"
    );
}

#[test]
fn headless_outgoing_column_stop_is_immediate_and_does_not_stop_incoming_video() {
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let clocks = sources.map(|source| clock(&engine, source.media));
    for control in &clocks {
        control.prepare(Duration::from_secs(10)).unwrap();
    }
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    for (column, source) in columns.iter().zip(sources) {
        assign(&mut engine, matrix.media, row, *column, source.media);
    }
    cut(&mut engine, matrix.media, columns[0]);
    configured(&mut engine, matrix.media, columns[1], 30_000);
    assert!(clocks.iter().all(|control| control.sample().unwrap().playing));
    super::controls::pulse(&mut engine, columns[0].0, "stop");
    golden_core::application::service_script_app_operations(&mut engine).unwrap();
    tick(&mut engine);
    assert_eq!(clocks[0].settings().unwrap().mode, VideoPlaybackMode::Stopped);
    assert!(clocks[1].sample().unwrap().playing);
}

#[test]
fn stale_fade_wakes_cannot_stop_reactivated_or_manually_overridden_video() {
    for reactivate in [false, true] {
        let mut engine = fixture();
        let video = create(&mut engine, MediaKind::Video);
        let control = clock(&engine, video.media);
        control.prepare(Duration::from_secs(10)).unwrap();
        let matrix = create(&mut engine, MediaKind::Matrix);
        let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
        let columns = columns(&mut engine, matrix.media, 2);
        assign(&mut engine, matrix.media, row, columns[0], video.media);
        cut(&mut engine, matrix.media, columns[0]);
        configured(&mut engine, matrix.media, columns[1], 40);
        if reactivate {
            configured(&mut engine, matrix.media, columns[0], 40);
        } else {
            policy(&mut engine, video.media, VideoEdit::Seek { position_ms: 3000.0 });
        }
        std::thread::sleep(Duration::from_millis(60));
        tick(&mut engine);
        assert!(control.sample().unwrap().playing);
        if !reactivate {
            assert!(control.sample().unwrap().position >= Duration::from_secs(3));
        }
    }
}

#[test]
fn matrix_disable_withdraws_shared_use_and_reenable_applies_the_current_activation() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    assign(&mut engine, matrix.media, row, columns[0], video.media);
    configured(&mut engine, matrix.media, columns[0], 5000);
    for enabled in [false, true, false] {
        let owner = engine.node_id_by_uuid(matrix.media.0).unwrap();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: owner,
                patch: golden_core::ui_sync::NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                },
            },
        );
        assert!(ack.success, "{ack:?}");
        tick(&mut engine);
        assert_eq!(control.sample().unwrap().playing, enabled);
    }
    // Disabled trigger owners do not replay or change their committed destination.
    configured(&mut engine, matrix.media, columns[1], 5000);
    let owner = engine.node_id_by_uuid(matrix.media.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: owner,
                patch: golden_core::ui_sync::NodeMetaPatch {
                    enabled: Some(true),
                    ..Default::default()
                },
            }
        )
        .success
    );
    tick(&mut engine);
    assert!(control.sample().unwrap().playing);
    assert_eq!(
        super::controls::current(&engine, matrix.media).desired_column,
        Some(columns[0])
    );
    edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    tick(&mut engine);
    assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn headless_layer_stop_withdraws_only_its_video_and_cancels_its_deadline() {
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let clocks = sources.map(|source| clock(&engine, source.media));
    for control in &clocks {
        control.prepare(Duration::from_secs(10)).unwrap();
    }
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let column = columns(&mut engine, matrix.media, 1)[0];
    for (row, source) in rows.iter().zip(sources) {
        assign(&mut engine, matrix.media, *row, column, source.media);
    }
    configured(&mut engine, matrix.media, column, 5000);
    assert!(clocks.iter().all(|control| control.sample().unwrap().playing));
    super::controls::pulse(&mut engine, rows[0].0, "stop");
    golden_core::application::service_script_app_operations(&mut engine).unwrap();
    tick(&mut engine);
    assert_eq!(clocks[0].settings().unwrap().mode, VideoPlaybackMode::Stopped);
    assert!(clocks[1].sample().unwrap().playing);
}

#[test]
fn headless_interruption_rejects_the_thirty_third_use_before_activation_and_cut_releases_it() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 33);
    let mut clocks = Vec::new();
    for column in &columns {
        let video = create(&mut engine, MediaKind::Video);
        let control = clock(&engine, video.media);
        control.prepare(Duration::from_secs(10)).unwrap();
        assign(&mut engine, matrix.media, row, *column, video.media);
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::ConfigureColumn {
                column: *column,
                definition: ColumnDefinition {
                    transition: Some(TransitionDefinition {
                        duration_ms: 30_000,
                        curve: TransitionCurve::Linear,
                    }),
                },
            },
        );
        clocks.push(control);
    }
    for column in &columns[..32] {
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::LaunchColumn {
                column: *column,
                mode: LaunchMode::Configured,
            },
        );
        tick(&mut engine);
    }
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::LaunchColumn {
                column: columns[32],
                mode: LaunchMode::Configured,
            },
        },
    );
    assert!(ack.success, "the public operation queues its typed trigger");
    tick(&mut engine);
    assert!(
        super::super::playback::launch(&mut engine, matrix.media, columns[32], LaunchMode::Configured)
            .unwrap_err()
            .contains("32 retained uses")
    );
    assert!(clocks[..32].iter().all(|control| control.sample().unwrap().playing));
    assert_eq!(clocks[32].settings().unwrap().mode, VideoPlaybackMode::Paused);
    cut(&mut engine, matrix.media, columns[32]);
    assert!(clocks[32].sample().unwrap().playing);
    assert!(
        clocks[..32]
            .iter()
            .all(|control| control.settings().unwrap().mode == VideoPlaybackMode::Stopped)
    );
}
