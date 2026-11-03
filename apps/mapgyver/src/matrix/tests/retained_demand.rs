use super::*;
use crate::applications::{demand::DemandReason, logical::capture_logical_applications, protocol::*};
use golden_core::{node::Node, ui_sync::NodeMetaPatch};
use golden_time::{MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority, TimeSnapshot};
use std::collections::HashSet;

struct Clock;
impl golden_time::Clock for Clock {
    fn now(&self) -> Result<MonotonicTime, golden_time::TimeError> {
        Ok(MonotonicTime::from_nanos(0))
    }
}
fn time() -> TimeSnapshot {
    TimeAuthority::new(Clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap()
}
fn at(time: TimeSnapshot, millis: u64) -> TimeSnapshot {
    time.at_monotonic(MonotonicTime::from_nanos(millis * 1_000_000))
        .unwrap()
}
fn ids(capture: &crate::media::CapturedChannels) -> HashSet<ApplicationId> {
    capture
        .application_demand()
        .iter()
        .map(|entry| entry.application)
        .collect()
}
fn playback(engine: &AppEngine, media: MediaId) -> MatrixPlaybackSnapshot {
    let MediaEditResult::MatrixPlayback { snapshot } = crate::matrix::playback::inspect(engine, media).unwrap() else {
        panic!()
    };
    snapshot
}

fn configure(engine: &mut AppEngine, media: MediaId, row: LayerId, column: ColumnId) -> (CellId, ApplicationId) {
    let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
        engine,
        media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column,
            definition: ApplicationDefinition {
                executable: Some("C:\\missing\\not-launched.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    ) else {
        panic!()
    };
    (cell, application)
}

#[test]
fn skipped_disabled_destination_keeps_column_origin_cell_demand_without_a_render_consumer() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let columns = columns(&mut engine, media, 2);
    let (_, application) = configure(&mut engine, media, row, columns[0]);
    let definition = crate::applications::inspect(&engine, application).unwrap().definition;
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumnApplication {
            column: columns[0],
            definition,
            pre: 0,
            post: 0,
        },
    );
    let column_application = ApplicationId(columns[0].0);
    let (disabled, _) = configure(&mut engine, media, row, columns[1]);
    let node = engine.node_id_by_uuid(disabled.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            }
        )
        .success
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    assert_eq!(capture_logical_applications(&engine).unwrap().demand.len(), 2);
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    let capture = capture_logical_applications(&engine).unwrap();
    assert_eq!(
        capture
            .demand
            .iter()
            .map(|entry| entry.application)
            .collect::<HashSet<_>>(),
        [application, column_application].into_iter().collect(),
        "skipping a disabled Cell keeps its previous active controls"
    );
    assert!(
        capture
            .demand
            .iter()
            .all(|entry| entry.reasons.contains(&DemandReason::Preparation))
    );
    let root = crate::matrix::playback::root(&engine, media).unwrap();
    let AppNode::MediaChannel(root) = engine.nodes.get(root).unwrap() else {
        panic!()
    };
    assert!(root.matrix_playback().unwrap().cached().is_none());
    edit(&mut engine, media, MatrixEdit::ClearPlayback);
    assert!(capture_logical_applications(&engine).unwrap().demand.is_empty());
}

#[test]
fn render_disabled_and_transparent_layers_keep_direct_app_demand_and_history_project_stays_authored_only() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, reference.media, None, 1)[0]);
    let columns = columns(&mut engine, reference.media, 2);
    let (_, application) = configure(&mut engine, reference.media, row, columns[0]);
    let definition = crate::applications::inspect(&engine, application).unwrap().definition;
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::ConfigureColumnApplication {
            column: columns[0],
            definition,
            pre: 0,
            post: 0,
        },
    );
    for settings in [
        CompositeSettings {
            enabled: false,
            ..Default::default()
        },
        CompositeSettings {
            opacity: 0.0,
            ..Default::default()
        },
    ] {
        edit(
            &mut engine,
            reference.media,
            MatrixEdit::ConfigureItem {
                item: MatrixItemId::Layer { id: row },
                settings,
            },
        );
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        edit(
            &mut engine,
            reference.media,
            MatrixEdit::LaunchCell {
                layer: row,
                column: columns[0],
                mode: LaunchMode::Cut,
            },
        );
        let logical = capture_logical_applications(&engine).unwrap();
        assert_eq!(logical.demand.len(), 1);
        assert_eq!(
            logical.demand[0].application, application,
            "individual activation does not start the Column control"
        );
        let capture = crate::media::capture_channels_at(&engine, &[reference], 16, time()).unwrap();
        assert_eq!(ids(&capture), [application].into_iter().collect());
        assert_eq!(capture.plan_applications(), [application]);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        assert!(capture_logical_applications(&loaded).unwrap().demand.is_empty());
        assert_eq!(playback(&loaded, reference.media), MatrixPlaybackSnapshot::default());
        assert!(engine.undo().unwrap());
        assert_eq!(
            capture_logical_applications(&engine).unwrap().demand[0].application,
            application
        );
    }
}

#[test]
fn independent_outgoing_deadlines_survive_interruption_reorder_deletion_undo_and_project_replacement() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, reference.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let columns = columns(&mut engine, reference.media, 4);
    let mut applications = Vec::new();
    for row in &rows {
        applications.push([
            configure(&mut engine, reference.media, *row, columns[0]).1,
            configure(&mut engine, reference.media, *row, columns[1]).1,
        ]);
    }
    // Different Cell overrides exercise independent retirement, not one global fade deadline.
    for (row, duration_ms) in [(rows[0], 100), (rows[1], 300)] {
        let cell = crate::matrix::document::Document::load(&engine, reference.media)
            .unwrap()
            .cells[&(row, columns[1])];
        let folder = engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().parent == Some(cell) && crate::parameters::is_parameter_folder(*node))
            .unwrap()
            .0;
        for (field, value) in [
            ("transition_override", golden_core::parameter::ParamValue::Bool(true)),
            (
                "duration_ms",
                golden_core::parameter::ParamValue::Float(duration_ms as f64),
            ),
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
                        behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
                    }
                )
                .success
            );
        }
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let now = time();
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::LaunchColumn {
            column: columns[2],
            mode: LaunchMode::Cut,
        },
    );
    for row in &rows {
        edit(
            &mut engine,
            reference.media,
            MatrixEdit::LaunchCell {
                layer: *row,
                column: columns[0],
                mode: LaunchMode::Cut,
            },
        );
    }
    let capture = |engine: &AppEngine, millis| {
        crate::media::capture_channels_at(engine, &[reference], 16, at(now, millis)).unwrap()
    };
    assert_eq!(
        ids(&capture(&engine, 0)),
        applications.iter().map(|pair| pair[0]).collect()
    );
    for row in &rows {
        edit(
            &mut engine,
            reference.media,
            MatrixEdit::LaunchCell {
                layer: *row,
                column: columns[1],
                mode: LaunchMode::Configured,
            },
        );
    }
    let fading = capture(&engine, 0);
    assert_eq!(ids(&fading), applications.iter().flatten().copied().collect());
    let mut settled = fading.clone();
    settled.settle_transitions(at(now, 100)).unwrap();
    assert_eq!(
        ids(&settled),
        [applications[0][1], applications[1][0], applications[1][1]]
            .into_iter()
            .collect()
    );
    assert_eq!(
        ids(&capture(&engine, 100)),
        [applications[0][1], applications[1][0], applications[1][1]]
            .into_iter()
            .collect()
    );
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::LaunchCell {
            layer: rows[1],
            column: columns[3],
            mode: LaunchMode::Configured,
        },
    );
    let interrupted = capture(&engine, 100);
    assert_eq!(
        ids(&interrupted),
        [applications[0][1], applications[1][0], applications[1][1]]
            .into_iter()
            .collect()
    );
    assert!(
        interrupted
            .application_demand()
            .iter()
            .filter(|entry| entry.application != applications[0][1])
            .all(|entry| entry.reasons.contains(&DemandReason::Outgoing))
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let requested = playback(&engine, reference.media);
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::ReorderColumn {
            column: columns[1],
            after: None,
        },
    );
    assert!(!interrupted.is_current());
    assert_eq!(ids(&capture(&engine, 125)), ids(&interrupted));
    assert!(engine.undo().unwrap());
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::RemoveColumn { column: columns[0] },
    );
    let deleted = capture(&engine, 150);
    let outgoing = deleted
        .application_demand()
        .into_iter()
        .find(|entry| entry.application == applications[1][0])
        .unwrap();
    assert_eq!(
        outgoing.availability,
        crate::applications::demand::DefinitionAvailability::Missing
    );
    assert!(outgoing.reasons.contains(&DemandReason::Outgoing));
    assert!(engine.undo().unwrap());
    assert_eq!(playback(&engine, reference.media), requested);
    assert_eq!(ids(&capture(&engine, 349)), ids(&interrupted));
    assert_eq!(ids(&capture(&engine, 350)), [applications[0][1]].into_iter().collect());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let old_project = capture_logical_applications(&engine).unwrap().project;
    engine = from_sparse_project_json::<AppNode>(&saved).unwrap();
    let loaded = capture_logical_applications(&engine).unwrap();
    assert_ne!(old_project, loaded.project);
    assert!(loaded.demand.is_empty());
    assert!(!deleted.is_current());
    assert!(capture(&engine, 400).plan_applications().is_empty());
}
