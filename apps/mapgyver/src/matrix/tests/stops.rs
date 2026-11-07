use super::*;
use super::{
    cell_playback::{at, capture, time},
    controls::{current, parameter, pulse},
};
use golden_core::{
    edit::Edit,
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use golden_time::TimeSnapshot;

struct Scene {
    engine: AppEngine,
    media: MediaId,
    rows: Vec<LayerId>,
    columns: Vec<ColumnId>,
    sources: std::collections::HashMap<ChannelReference, usize>,
}
impl Scene {
    fn new(row_count: u16) -> Self {
        let mut engine = fixture();
        let media = create(&mut engine, MediaKind::Matrix).media;
        let rows = layers(&mut engine, media, None, row_count)
            .into_iter()
            .map(layer)
            .collect::<Vec<_>>();
        let columns = columns(&mut engine, media, 3);
        let sources = (0..3)
            .map(|_| create(&mut engine, MediaKind::SolidColor))
            .collect::<Vec<_>>();
        for row in &rows {
            for (column, source) in columns.iter().zip(&sources) {
                edit(
                    &mut engine,
                    media,
                    MatrixEdit::DropSource {
                        layer: *row,
                        column: *column,
                        source: source.media.0,
                        copy: false,
                    },
                );
            }
        }
        let sources = inspect(&mut engine, media)
            .cells
            .into_iter()
            .map(|cell| {
                (
                    cell.definition.source.unwrap(),
                    columns
                        .iter()
                        .position(|column| *column == cell.definition.column)
                        .unwrap(),
                )
            })
            .collect();
        Self {
            engine,
            media,
            rows,
            columns,
            sources,
        }
    }
    fn launch(&mut self, column: usize, mode: LaunchMode) {
        edit(
            &mut self.engine,
            self.media,
            MatrixEdit::LaunchColumn {
                column: self.columns[column],
                mode,
            },
        );
    }
    fn weights(&self, row: usize, now: TimeSnapshot) -> Vec<(usize, f32)> {
        let plan = capture(&self.engine, self.media, self.rows[row], now);
        let mut weights = plan
            .inputs
            .iter()
            .map(|input| {
                (
                    self.sources[&input.source],
                    input.weight.map_or(1.0, |weight| weight.sample(now).get()),
                )
            })
            .collect::<Vec<_>>();
        weights.sort_by_key(|(source, _)| *source);
        weights
    }
    fn cell(&mut self, row: usize, column: usize) -> CellId {
        inspect(&mut self.engine, self.media)
            .cells
            .iter()
            .find(|cell| cell.layer == self.rows[row] && cell.definition.column == self.columns[column])
            .unwrap()
            .id
    }
}

#[test]
fn column_stop_cuts_only_matching_outgoing_clips_without_reanchoring_unrelated_fades() {
    let mut scene = Scene::new(2);
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    assert_eq!(scene.weights(0, now), [(0, 1.0)]);
    scene.launch(1, LaunchMode::Configured);
    scene.weights(0, now);
    let old = capture(&scene.engine, scene.media, scene.rows[0], at(now, 125));
    let authored = to_sparse_project_json_pretty(&scene.engine).unwrap();
    let history = scene.engine.undo_len();
    pulse(&mut scene.engine, scene.columns[0].0, "stop");
    assert_eq!(scene.weights(0, at(now, 125)), [(1, 0.5)]);
    assert_eq!(scene.weights(1, at(now, 125)), [(1, 0.5)]);
    assert_eq!(scene.weights(0, at(now, 250)), [(1, 1.0)]);
    assert_eq!(
        current(&scene.engine, scene.media).desired_column,
        Some(scene.columns[1])
    );
    assert_eq!(old.inputs.len(), 2);
    assert_eq!(scene.engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&scene.engine).unwrap(), authored);
}

#[test]
fn stopping_the_current_column_keeps_other_outgoing_envelopes_and_clears_the_anchor() {
    let mut scene = Scene::new(2);
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    scene.weights(0, now);
    scene.launch(1, LaunchMode::Configured);
    scene.weights(0, now);
    pulse(&mut scene.engine, scene.columns[1].0, "stop");
    assert_eq!(scene.weights(0, at(now, 125)), [(0, 0.5)]);
    assert_eq!(current(&scene.engine, scene.media).desired_column, None);
    let independent = scene.cell(1, 2);
    pulse(&mut scene.engine, independent.0, "cut");
    assert_eq!(scene.weights(0, at(now, 125)), [(0, 0.5)]);
    assert_eq!(scene.weights(0, at(now, 250)), []);
    assert_eq!(scene.weights(1, at(now, 250)), [(2, 1.0)]);
}

#[test]
fn cell_stop_preserves_other_addresses_and_layer_stop_clears_all_its_independent_clips() {
    let mut scene = Scene::new(2);
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    scene.weights(0, now);
    scene.launch(1, LaunchMode::Configured);
    scene.weights(0, now);
    let cell = scene.cell(0, 1);
    pulse(&mut scene.engine, cell.0, "stop");
    assert_eq!(scene.weights(0, at(now, 125)), [(0, 0.5)]);
    assert_eq!(scene.weights(1, at(now, 125)), [(0, 0.5), (1, 0.5)]);
    assert_eq!(
        current(&scene.engine, scene.media).desired_column,
        Some(scene.columns[1])
    );
    let independent = scene.cell(0, 2);
    pulse(&mut scene.engine, independent.0, "play");
    scene.weights(0, at(now, 125));
    pulse(&mut scene.engine, scene.rows[0].0, "stop");
    assert_eq!(scene.weights(0, at(now, 175)), []);
    assert_eq!(scene.weights(1, at(now, 175)), [(0, 0.3), (1, 0.7)]);
    assert_eq!(
        current(&scene.engine, scene.media).desired_column,
        Some(scene.columns[1])
    );
}

#[test]
fn unseen_column_and_row_stops_preserve_unrelated_mixes_and_a_later_play_survives() {
    let mut scene = Scene::new(2);
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    scene.weights(0, now);
    scene.launch(1, LaunchMode::Configured);
    pulse(&mut scene.engine, scene.columns[1].0, "stop");
    assert_eq!(scene.weights(0, now), [(0, 1.0)]);
    assert_eq!(scene.weights(1, now), [(0, 1.0)]);
    scene.launch(1, LaunchMode::Configured);
    pulse(&mut scene.engine, scene.rows[0].0, "stop");
    assert_eq!(scene.weights(0, now), []);
    assert_eq!(scene.weights(1, at(now, 125)), [(0, 0.5), (1, 0.5)]);
    let cell = scene.cell(0, 1);
    for _ in 0..20 {
        pulse(&mut scene.engine, cell.0, "stop");
        pulse(&mut scene.engine, cell.0, "cut");
    }
    assert_eq!(scene.weights(0, at(now, 125)), [(1, 1.0)]);
    assert_eq!(scene.weights(1, at(now, 125)), [(0, 0.5), (1, 0.5)]);
}

#[test]
fn nested_layer_and_column_group_stops_use_their_distinct_authored_membership() {
    let mut scene = Scene::new(3);
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: outer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::AddGroup {
            parent: Some(outer),
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: inner } = items[0] else {
        panic!()
    };
    for (row, parent) in [(scene.rows[0], outer), (scene.rows[1], inner)] {
        edit(
            &mut scene.engine,
            scene.media,
            MatrixEdit::MoveItem {
                item: MatrixItemId::Layer { id: row },
                parent: Some(parent),
                after: None,
            },
        );
    }
    let MediaEditResult::MatrixColumnGroupCreated { group } = edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::AddColumnGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixColumnGroupCreated { group: nested } = edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::AddColumnGroup {
            parent: Some(group),
            label: None,
        },
    ) else {
        panic!()
    };
    for (column, parent) in [(scene.columns[0], group), (scene.columns[1], nested)] {
        edit(
            &mut scene.engine,
            scene.media,
            MatrixEdit::MoveColumnItem {
                item: ColumnItemId::Column { id: column },
                parent: Some(parent),
                after: None,
            },
        );
    }
    let now = time();
    scene.launch(2, LaunchMode::Cut);
    let cell = scene.cell(0, 0);
    pulse(&mut scene.engine, cell.0, "cut");
    let cell = scene.cell(1, 1);
    pulse(&mut scene.engine, cell.0, "cut");
    scene.weights(0, now);
    pulse(&mut scene.engine, group.0, "stop");
    assert_eq!(scene.weights(0, now), []);
    assert_eq!(scene.weights(1, now), []);
    assert_eq!(scene.weights(2, now), [(2, 1.0)]);
    assert_eq!(
        current(&scene.engine, scene.media).desired_column,
        Some(scene.columns[2])
    );
    let cell = scene.cell(0, 2);
    pulse(&mut scene.engine, cell.0, "cut");
    let cell = scene.cell(1, 2);
    pulse(&mut scene.engine, cell.0, "cut");
    scene.weights(0, now);
    pulse(&mut scene.engine, outer.0, "stop");
    assert_eq!(scene.weights(0, now), []);
    assert_eq!(scene.weights(1, now), []);
    assert_eq!(scene.weights(2, now), [(2, 1.0)]);
}

#[test]
fn discarded_scoped_stop_is_invisible_and_disabled_control_owner_cancels_queued_stop() {
    let mut scene = Scene::new(1);
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    scene.weights(0, now);
    let before = current(&scene.engine, scene.media);
    super::super::stops::stop(&mut scene.engine, scene.media, scene.columns[0].0).unwrap();
    assert_eq!(current(&scene.engine, scene.media), before);
    scene.engine.edits.pending.clear();
    assert_eq!(current(&scene.engine, scene.media), before);
    assert_eq!(scene.weights(0, now), [(0, 1.0)]);
    let trigger = parameter(&scene.engine, scene.columns[0].0, "stop");
    scene.engine.edits.push(Edit::SetParam {
        node: trigger,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Append,
    });
    scene.engine.apply_edits().unwrap();
    let owner = scene.engine.node_id_by_uuid(scene.columns[0].0).unwrap();
    scene.engine.edits.push(Edit::PatchMeta {
        node: owner,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    scene.engine.apply_edits().unwrap();
    golden_core::application::service_script_app_operations(&mut scene.engine).unwrap();
    assert_eq!(current(&scene.engine, scene.media), before);
}

#[test]
fn stop_all_frees_interrupted_mix_admission_before_the_next_capture() {
    let mut scene = Scene::new(1);
    let more = columns(&mut scene.engine, scene.media, 33);
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    scene.weights(0, now);
    for (index, column) in more[..31].iter().enumerate() {
        edit(
            &mut scene.engine,
            scene.media,
            MatrixEdit::LaunchColumn {
                column: *column,
                mode: LaunchMode::Configured,
            },
        );
        capture(&scene.engine, scene.media, scene.rows[0], at(now, index as u64 + 1));
    }
    let rejected = invoke(
        &mut scene.engine,
        MediaEdit::Matrix {
            media: scene.media,
            edit: MatrixEdit::LaunchColumn {
                column: more[31],
                mode: LaunchMode::Configured,
            },
        },
    );
    assert!(
        rejected.success,
        "the Parameter pulse is acknowledged independently of playback admission"
    );
    assert_ne!(current(&scene.engine, scene.media).desired_column, Some(more[31]));
    let owner = scene.engine.node_id_by_uuid(more[31].0).unwrap();
    assert!(
        !scene
            .engine
            .nodes
            .get(owner)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .warnings
            .is_empty()
    );
    pulse(&mut scene.engine, scene.media.0, "stop_all");
    edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::LaunchColumn {
            column: more[31],
            mode: LaunchMode::Configured,
        },
    );
    let plan = capture(&scene.engine, scene.media, scene.rows[0], at(now, 32));
    assert!(
        plan.inputs.is_empty(),
        "stopped historical sources cannot survive the new empty Column"
    );
    assert_eq!(current(&scene.engine, scene.media).desired_column, Some(more[31]));
}

#[test]
fn scoped_stops_clear_playing_references_preserve_anchor_demand_and_disabled_play_is_a_noop() {
    let mut scene = Scene::new(2);
    let mut owners = Vec::new();
    for row in &scene.rows {
        let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
            &mut scene.engine,
            scene.media,
            MatrixEdit::ConfigureCellApplication {
                layer: *row,
                column: scene.columns[0],
                definition: crate::applications::protocol::ApplicationDefinition {
                    executable: Some("C:\\missing\\shared-not-launched.exe".into()),
                    ..Default::default()
                },
                pre: 0,
                post: 0,
            },
        ) else {
            panic!()
        };
        owners.push((cell, application));
    }
    scene.launch(0, LaunchMode::Cut);
    let now = time();
    let old = capture(&scene.engine, scene.media, scene.rows[0], now);
    assert!(old.applications.references.contains(&owners[0].1));
    let authored = to_sparse_project_json_pretty(&scene.engine).unwrap();
    let history = scene.engine.undo_len();
    pulse(&mut scene.engine, scene.rows[0].0, "stop");
    let logical = crate::applications::logical::capture_logical_applications(&scene.engine).unwrap();
    assert!(
        capture(&scene.engine, scene.media, scene.rows[0], now)
            .applications
            .references
            .is_empty()
    );
    assert!(
        capture(&scene.engine, scene.media, scene.rows[1], now)
            .applications
            .references
            .contains(&owners[1].1)
    );
    // The authored Column anchor is intentionally retained: its preparation/pre/post
    // input is still a valid aggregate lease, independently of the stopped playing Cell.
    assert_eq!(logical.demand.len(), 2);
    pulse(&mut scene.engine, owners[1].0.0, "stop");
    assert!(
        capture(&scene.engine, scene.media, scene.rows[1], now)
            .applications
            .references
            .is_empty()
    );
    assert_eq!(
        crate::applications::logical::capture_logical_applications(&scene.engine)
            .unwrap()
            .demand
            .len(),
        2
    );
    pulse(&mut scene.engine, scene.media.0, "stop_all");
    assert!(
        crate::applications::logical::capture_logical_applications(&scene.engine)
            .unwrap()
            .demand
            .is_empty()
    );
    assert!(old.applications.references.contains(&owners[0].1));
    assert_eq!(scene.engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&scene.engine).unwrap(), authored);
    edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::ConfigureCell {
            layer: scene.rows[0],
            column: scene.columns[0],
            settings: CellSettings {
                enabled: false,
                ..Default::default()
            },
        },
    );
    let before = current(&scene.engine, scene.media);
    pulse(&mut scene.engine, owners[0].0.0, "play");
    assert_eq!(current(&scene.engine, scene.media), before);
    pulse(&mut scene.engine, owners[0].0.0, "stop");
    assert!(
        crate::applications::logical::capture_logical_applications(&scene.engine)
            .unwrap()
            .demand
            .is_empty()
    );
}

#[test]
fn group_stop_does_not_materialize_runtime_requests_for_inactive_configured_rows() {
    let mut scene = Scene::new(3);
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: group } = items[0] else {
        panic!()
    };
    for row in &scene.rows {
        edit(
            &mut scene.engine,
            scene.media,
            MatrixEdit::MoveItem {
                item: MatrixItemId::Layer { id: *row },
                parent: Some(group),
                after: None,
            },
        );
    }
    edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::SetCells {
            assignments: scene.rows[1..]
                .iter()
                .map(|row| CellAssignment {
                    layer: *row,
                    column: scene.columns[0],
                    source: None,
                })
                .collect(),
        },
    );
    let now = time();
    scene.launch(0, LaunchMode::Cut);
    assert_eq!(scene.weights(0, now), [(0, 1.0)]);
    assert_eq!(scene.weights(1, now), []);
    pulse(&mut scene.engine, group.0, "stop");
    let stopped = current(&scene.engine, scene.media);
    assert_eq!(stopped.layers.len(), 1);
    assert_eq!(stopped.layers[0].layer, scene.rows[0]);
    assert_eq!(scene.weights(0, now), []);
    scene.launch(1, LaunchMode::Cut);
    assert_eq!(scene.weights(1, now), [(1, 1.0)]);
    assert_eq!(scene.weights(2, now), [(1, 1.0)]);
}
