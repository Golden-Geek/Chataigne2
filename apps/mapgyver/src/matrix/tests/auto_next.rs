use super::{
    controls::{current, parameter},
    video::{assign, clock, configured, cut, policy, tick},
    *,
};
use crate::media::video::protocol::VideoEdit;
use golden_core::{
    application::service_script_app_operations,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::time::{Duration, Instant};

fn setting(engine: &mut AppEngine, cell: CellId, field: &str, value: ParamValue) {
    let node = parameter(engine, cell.0, field);
    let result = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(result.success, "{result:?}");
}

fn settle(engine: &mut AppEngine) {
    tick(engine);
    service_script_app_operations(engine).unwrap();
}

fn wait_for(engine: &mut AppEngine, span: Duration, mut condition: impl FnMut(&AppEngine) -> bool) -> bool {
    let until = Instant::now() + span;
    loop {
        settle(engine);
        if condition(engine) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn fixture_with_cells(source_kind: MediaKind) -> (AppEngine, MediaId, MediaId, LayerId, [ColumnId; 2], CellId) {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let source = create(&mut engine, source_kind).media;
    let next = create(&mut engine, MediaKind::SolidColor).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let columns: [ColumnId; 2] = columns(&mut engine, media, 2).try_into().unwrap();
    assign(&mut engine, media, row, columns[0], source);
    assign(&mut engine, media, row, columns[1], next);
    let cell = inspect(&mut engine, media)
        .cells
        .into_iter()
        .find(|cell| cell.definition.column == columns[0])
        .unwrap()
        .id;
    (engine, media, source, row, columns, cell)
}

fn has_cell(engine: &AppEngine, media: MediaId, row: LayerId, column: ColumnId) -> bool {
    current(engine, media)
        .layers
        .iter()
        .any(|request| request.layer == row && request.column == Some(column))
}

#[test]
fn auto_next_timed_respects_disabled_mode_and_stop_without_authoring_history() {
    let (mut engine, media, _, row, [first, next], cell) = fixture_with_cells(MediaKind::SolidColor);
    setting(&mut engine, cell, "auto_next_time_ms", ParamValue::Float(30.0));
    cut(&mut engine, media, first);
    assert!(!wait_for(&mut engine, Duration::from_millis(80), |engine| has_cell(
        engine, media, row, next
    )));

    setting(&mut engine, cell, "auto_next", ParamValue::Enum("Specific Time".into()));
    let history = engine.undo_len();
    assert!(wait_for(&mut engine, Duration::from_secs(2), |engine| has_cell(
        engine, media, row, next
    )));
    assert_eq!(current(&engine, media).desired_column, Some(first));
    assert_eq!(engine.undo_len(), history);

    setting(&mut engine, cell, "auto_next_time_ms", ParamValue::Float(250.0));
    cut(&mut engine, media, first);
    settle(&mut engine);
    edit(&mut engine, media, MatrixEdit::ClearPlayback);
    assert!(!wait_for(&mut engine, Duration::from_millis(350), |engine| has_cell(
        engine, media, row, next
    )));
    assert!(current(&engine, media).desired_column.is_none());
}

#[test]
fn auto_next_crop_endpoint_and_transition_lead_use_the_live_video_clock() {
    for include_transition in [false, true] {
        let (mut engine, media, video, row, [first, next], cell) = fixture_with_cells(MediaKind::Video);
        let clock = clock(&engine, video);
        clock.prepare(Duration::from_secs(10)).unwrap();
        policy(
            &mut engine,
            video,
            VideoEdit::Crop {
                start_ms: 2000.0,
                end_ms: Some(2600.0),
            },
        );
        policy(&mut engine, video, VideoEdit::Speed { speed: 0.0 });
        configured(&mut engine, media, next, 1200);
        setting(&mut engine, cell, "auto_next", ParamValue::Enum("End of Media".into()));
        setting(
            &mut engine,
            cell,
            "auto_next_lead",
            ParamValue::Bool(include_transition),
        );
        let history = engine.undo_len();
        cut(&mut engine, media, first);
        assert!(!wait_for(&mut engine, Duration::from_millis(80), |engine| has_cell(
            engine, media, row, next
        )));
        policy(&mut engine, video, VideoEdit::Speed { speed: 1.0 });
        assert!(wait_for(&mut engine, Duration::from_secs(2), |engine| has_cell(
            engine, media, row, next
        )));
        let sampled = clock.sample().unwrap();
        assert_eq!(sampled.duration, Duration::from_millis(600));
        assert_eq!(sampled.playing, include_transition);
        if !include_transition {
            assert_eq!(sampled.position, sampled.duration);
        }
        let snapshot = current(&engine, media);
        assert_eq!(snapshot.desired_column, Some(first));
        assert_eq!(
            snapshot
                .layers
                .iter()
                .find(|request| request.layer == row)
                .unwrap()
                .transition
                .duration_ms,
            1200
        );
        assert_eq!(
            engine.undo_len(),
            history + 1,
            "Only the authored Video Speed edit changes history"
        );
    }
}
