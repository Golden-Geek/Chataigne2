use super::{
    column_groups::{group, nested_columns},
    controls::{current, parameter, pulse},
    video::{assign, clock, cut, policy, tick},
    *,
};
use crate::media::video::protocol::VideoEdit;
use golden_core::{
    application::service_script_app_operations,
    node::NodeUuid,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::time::{Duration, Instant};

fn wait_mode(engine: &mut AppEngine, owner: NodeUuid, mode: &str) {
    let node = parameter(engine, owner, "navigation_wait");
    let result = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value: ParamValue::Enum(mode.into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(result.success, "{result:?}");
}

fn observe(engine: &mut AppEngine, span: Duration, condition: impl Fn(&AppEngine) -> bool) -> bool {
    let until = Instant::now() + span;
    loop {
        tick(engine);
        service_script_app_operations(engine).unwrap();
        if condition(engine) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn has_cell(engine: &AppEngine, matrix: MediaId, layer: LayerId, column: ColumnId) -> bool {
    current(engine, matrix)
        .layers
        .iter()
        .any(|request| request.layer == layer && request.column == Some(column))
}

#[test]
fn matrix_wait_modes_use_the_earliest_or_latest_active_end() {
    for (mode, long_still_playing) in [("First Media End", true), ("All Media End", false)] {
        let mut engine = fixture();
        let matrix = create(&mut engine, MediaKind::Matrix).media;
        let short = create(&mut engine, MediaKind::Video).media;
        let long = create(&mut engine, MediaKind::Video).media;
        let rows = layers(&mut engine, matrix, None, 2);
        let [first, next]: [ColumnId; 2] = columns(&mut engine, matrix, 2).try_into().unwrap();
        assign(&mut engine, matrix, layer(rows[0]), first, short);
        assign(&mut engine, matrix, layer(rows[1]), first, long);
        clock(&engine, short).prepare(Duration::from_millis(350)).unwrap();
        let long_clock = clock(&engine, long);
        long_clock.prepare(Duration::from_millis(1450)).unwrap();
        wait_mode(&mut engine, matrix.0, mode);
        let history = engine.undo_len();

        cut(&mut engine, matrix, first);
        edit(
            &mut engine,
            matrix,
            MatrixEdit::NavigateColumn {
                direction: MatrixNavigationDirection::Next,
            },
        );
        assert_eq!(current(&engine, matrix).desired_column, Some(first), "{mode}");
        assert!(observe(&mut engine, Duration::from_secs(3), |engine| {
            current(engine, matrix).desired_column == Some(next)
        }));
        assert_eq!(long_clock.sample().unwrap().playing, long_still_playing, "{mode}");
        assert_eq!(history, engine.undo_len());
    }
}

#[test]
fn layer_wait_can_be_replaced_and_is_cancelled_by_stop_or_column_launch() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let video = create(&mut engine, MediaKind::Video).media;
    let fill = create(&mut engine, MediaKind::SolidColor).media;
    let row = layer(layers(&mut engine, matrix, None, 1)[0]);
    let [previous, first, next]: [ColumnId; 3] = columns(&mut engine, matrix, 3).try_into().unwrap();
    assign(&mut engine, matrix, row, previous, fill);
    assign(&mut engine, matrix, row, first, video);
    assign(&mut engine, matrix, row, next, fill);
    clock(&engine, video).prepare(Duration::from_millis(650)).unwrap();
    wait_mode(&mut engine, row.0, "All Media End");

    cut(&mut engine, matrix, first);
    pulse(&mut engine, row.0, "next_cell");
    pulse(&mut engine, row.0, "previous_cell");
    assert!(!has_cell(&engine, matrix, row, previous));
    assert!(observe(&mut engine, Duration::from_secs(2), |engine| {
        has_cell(engine, matrix, row, previous)
    }));
    assert_eq!(current(&engine, matrix).desired_column, Some(first));

    cut(&mut engine, matrix, first);
    pulse(&mut engine, row.0, "next_cell");
    pulse(&mut engine, row.0, "stop");
    assert!(!observe(&mut engine, Duration::from_millis(750), |engine| {
        has_cell(engine, matrix, row, next)
    }));
    assert!(
        current(&engine, matrix)
            .layers
            .iter()
            .any(|r| r.layer == row && r.column.is_none())
    );

    cut(&mut engine, matrix, first);
    pulse(&mut engine, row.0, "next_cell");
    cut(&mut engine, matrix, previous);
    assert!(!observe(&mut engine, Duration::from_millis(750), |engine| {
        has_cell(engine, matrix, row, next)
    }));
    assert_eq!(current(&engine, matrix).desired_column, Some(previous));
}

#[test]
fn group_wait_uses_its_own_finite_media_and_ignores_looping_clips() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let video = create(&mut engine, MediaKind::Video).media;
    let peer = create(&mut engine, MediaKind::Video).media;
    let rows = layers(&mut engine, matrix, None, 2);
    let [row, peer_row] = [layer(rows[0]), layer(rows[1])];
    let outside = columns(&mut engine, matrix, 1)[0];
    let parent = group(&mut engine, matrix, None);
    let [first, next]: [ColumnId; 2] = nested_columns(&mut engine, matrix, parent, 2).try_into().unwrap();
    assign(&mut engine, matrix, row, first, video);
    assign(&mut engine, matrix, peer_row, outside, peer);
    clock(&engine, video).prepare(Duration::from_millis(550)).unwrap();
    clock(&engine, peer).prepare(Duration::from_secs(15)).unwrap();
    wait_mode(&mut engine, parent.0, "All Media End");

    for looping in [false, true] {
        policy(&mut engine, video, VideoEdit::Loop { looping });
        cut(&mut engine, matrix, first);
        edit(
            &mut engine,
            matrix,
            MatrixEdit::LaunchCell {
                layer: peer_row,
                column: outside,
                mode: LaunchMode::Cut,
            },
        );
        tick(&mut engine);
        assert!(clock(&engine, peer).sample().unwrap().playing);
        pulse(&mut engine, parent.0, "next_child");
        let target = if looping { next } else { first };
        assert_eq!(current(&engine, matrix).desired_column, Some(target));
        assert!(observe(&mut engine, Duration::from_secs(2), |engine| {
            current(engine, matrix).desired_column == Some(next)
        }));
    }
}
