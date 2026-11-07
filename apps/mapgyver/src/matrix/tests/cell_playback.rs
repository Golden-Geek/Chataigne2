use super::*;
use golden_core::{
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use golden_time::{MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority, TimeSnapshot};
use std::collections::HashSet;

struct Clock;
impl golden_time::Clock for Clock {
    fn now(&self) -> Result<MonotonicTime, golden_time::TimeError> {
        Ok(MonotonicTime::from_nanos(0))
    }
}

#[test]
fn direct_cell_outgoing_app_reference_survives_column_deletion_until_its_own_fade_finishes() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 3);
    let mut applications = Vec::new();
    for (index, column) in columns[..2].iter().enumerate() {
        let MediaEditResult::MatrixCellPrepared { application, .. } = edit(
            &mut engine,
            matrix.media,
            MatrixEdit::ConfigureCellApplication {
                layer: row,
                column: *column,
                definition: crate::applications::protocol::ApplicationDefinition {
                    executable: Some("C:\\missing\\not-launched.exe".into()),
                    arguments: vec![index.to_string()],
                    ..Default::default()
                },
                pre: 0,
                post: 0,
            },
        ) else {
            panic!()
        };
        applications.push(application);
    }
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[2],
            mode: LaunchMode::Cut,
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    assert_eq!(
        &*capture(&engine, matrix.media, row, now).applications.references,
        &[applications[0]]
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    assert_eq!(
        capture(&engine, matrix.media, row, now).applications.references.len(),
        2
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RemoveColumn { column: columns[0] },
    );
    let middle = capture(&engine, matrix.media, row, at(now, 125));
    assert!(middle.applications.references.contains(&applications[0]));
    assert!(
        middle
            .applications
            .demand
            .iter()
            .find(|entry| entry.application == applications[0])
            .unwrap()
            .reasons
            .contains(&crate::applications::demand::DemandReason::Outgoing)
    );
    assert_eq!(
        &*capture(&engine, matrix.media, row, at(now, 250))
            .applications
            .references,
        &[applications[1]]
    );
}

#[test]
fn disabled_column_cell_holds_previous_layer_content_until_an_enabled_launch() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    let a = create(&mut engine, MediaKind::SolidColor);
    let b = create(&mut engine, MediaKind::SolidColor);
    for (column, source) in [(columns[0], a), (columns[1], b)] {
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::DropSource {
                layer: row,
                column,
                source: source.media.0,
                copy: false,
            },
        );
    }
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let old = capture(&engine, matrix.media, row, now).inputs[0].source;
    let cell = crate::matrix::document::Document::load(&engine, matrix.media)
        .unwrap()
        .cells[&(row, columns[1])];
    for enabled in [false, true] {
        assert!(
            apply_project_intent(
                &mut engine,
                UiEditIntent::PatchMeta {
                    node: cell,
                    patch: golden_core::ui_sync::NodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    }
                }
            )
            .success
        );
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::LaunchColumn {
                column: columns[1],
                mode: LaunchMode::Cut,
            },
        );
        let actual = capture(&engine, matrix.media, row, now);
        assert_eq!(actual.inputs.len(), 1);
        assert_eq!(actual.inputs[0].source == old, !enabled);
    }
}
pub(super) fn time() -> TimeSnapshot {
    TimeAuthority::new(Clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap()
}
pub(super) fn at(time: TimeSnapshot, millis: u64) -> TimeSnapshot {
    time.at_monotonic(MonotonicTime::from_nanos(millis * 1_000_000))
        .unwrap()
}
pub(super) fn capture(
    engine: &AppEngine,
    media: MediaId,
    row: LayerId,
    time: TimeSnapshot,
) -> crate::matrix::capture::CapturedMatrix {
    let snapshot = crate::matrix::document::Document::load(engine, media).unwrap().snapshot;
    let reference = snapshot
        .items
        .iter()
        .find(|item| item.id == MatrixItemId::Layer { id: row })
        .unwrap()
        .channel;
    crate::matrix::capture::capture(
        engine,
        reference,
        &MatrixOutput::Layer { layer: row },
        &mut 4096,
        &mut HashSet::new(),
        &mut Vec::new(),
        Some(time),
    )
    .unwrap()
    .unwrap()
}
#[test]
fn direct_cell_blends_only_its_layer_preserves_anchor_and_next_column_is_atomic_from_actual_mixes() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let columns = columns(&mut engine, matrix.media, 3);
    let a = create(&mut engine, MediaKind::SolidColor);
    let b = create(&mut engine, MediaKind::SolidColor);
    for (row, column, source) in [
        (rows[0], columns[0], a),
        (rows[1], columns[0], a),
        (rows[0], columns[1], b),
        (rows[1], columns[2], b),
    ] {
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::DropSource {
                layer: row,
                column,
                source: source.media.0,
                copy: false,
            },
        );
    }
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let time = time();
    let first = capture(&engine, matrix.media, rows[0], time);
    let peer = capture(&engine, matrix.media, rows[1], time);
    assert_eq!(first.inputs.len(), 1);
    let old = first.inputs[0].source;
    let result = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[0],
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    assert!(matches!(result, MediaEditResult::MatrixControlInvoked { .. }));
    let MediaEditResult::MatrixPlayback { snapshot } = super::super::playback::inspect(&engine, matrix.media).unwrap()
    else {
        panic!()
    };
    assert_eq!(snapshot.desired_column, Some(columns[0]));
    assert_eq!(snapshot.layers.len(), 1);
    let fade = capture(&engine, matrix.media, rows[0], time);
    assert_eq!(fade.inputs.len(), 2);
    let midpoint = at(time, 125);
    assert!(
        fade.inputs
            .iter()
            .all(|input| (input.weight.unwrap().sample(midpoint).get() - 0.5).abs() < 0.0001)
    );
    assert_eq!(
        capture(&engine, matrix.media, rows[1], midpoint).inputs[0].source,
        peer.inputs[0].source
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[2],
            mode: LaunchMode::Configured,
        },
    );
    let interrupted = capture(&engine, matrix.media, rows[0], midpoint);
    assert_eq!(
        interrupted.inputs.len(),
        2,
        "empty destination fades both visible participants out"
    );
    assert!(
        interrupted
            .inputs
            .iter()
            .all(|input| (input.weight.unwrap().sample(midpoint).get() - 0.5).abs() < 0.0001)
    );
    assert!(interrupted.inputs.iter().any(|input| input.source == old));
    assert!(capture(&engine, matrix.media, rows[0], at(time, 375)).inputs.is_empty());
    assert_eq!(capture(&engine, matrix.media, rows[1], at(time, 375)).inputs.len(), 1);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn cell_override_and_disablement_are_typed_and_logical_direct_demand_works_without_an_anchor_or_renderer() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column: columns[1],
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\not-launched.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    ) else {
        panic!()
    };
    let owner = engine.node_id_by_uuid(cell.0).unwrap();
    let folder = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(owner) && crate::parameters::is_parameter_folder(*node))
        .unwrap()
        .0;
    for (field, value) in [
        ("transition_override", ParamValue::Bool(true)),
        ("duration_ms", ParamValue::Float(17.0)),
    ] {
        let node = engine
            .nodes
            .iter()
            .find(|(_, node)| {
                node.node_data().parent == Some(folder)
                    && node.node_data().meta.decl_id.0 == format!("parameters/{field}")
            })
            .unwrap()
            .0;
        assert!(
            apply_project_intent(
                &mut engine,
                UiEditIntent::SetParam {
                    node,
                    value,
                    behaviour: ParameterEventBehaviour::Append
                }
            )
            .success
        );
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let result = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    assert!(matches!(result, MediaEditResult::MatrixControlInvoked { .. }));
    let MediaEditResult::MatrixPlayback { snapshot } = super::super::playback::inspect(&engine, matrix.media).unwrap()
    else {
        panic!()
    };
    assert_eq!(snapshot.desired_column, None);
    assert_eq!(snapshot.layers[0].transition.duration_ms, 17);
    let logical = crate::applications::logical::capture_logical_applications(&engine).unwrap();
    assert_eq!(logical.demand.len(), 1);
    assert_eq!(logical.demand[0].application, application);
    assert!(crate::matrix::document::Document::load(&engine, matrix.media).is_ok());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node: owner,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(false),
                ..Default::default()
            },
        },
    );
    assert!(ack.success);
    let result = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    assert!(matches!(result, MediaEditResult::MatrixControlInvoked { .. }));
    let before = crate::matrix::playback::inspect(&engine, matrix.media).unwrap();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    assert_eq!(
        serde_json::to_value(crate::matrix::playback::inspect(&engine, matrix.media).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}
