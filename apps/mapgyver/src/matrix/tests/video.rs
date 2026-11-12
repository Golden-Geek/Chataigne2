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
