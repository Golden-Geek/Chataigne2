use super::*;

fn records(project: &str) -> std::collections::BTreeMap<String, (Option<String>, serde_json::Value)> {
    fn visit(
        mut node: serde_json::Value,
        parent: Option<String>,
        result: &mut std::collections::BTreeMap<String, (Option<String>, serde_json::Value)>,
    ) {
        let uuid = node["uuid"].as_str().unwrap().to_owned();
        let children = node.as_object_mut().unwrap().remove("children");
        if let Some(serde_json::Value::Array(children)) = children {
            for child in children {
                visit(child, Some(uuid.clone()), result);
            }
        }
        assert!(result.insert(uuid, (parent, node)).is_none());
    }
    let project: serde_json::Value = serde_json::from_str(project).unwrap();
    let mut result = std::collections::BTreeMap::new();
    visit(project["root"].clone(), None, &mut result);
    result
}

fn owned(engine: &mut AppEngine, media: MediaId, layer: LayerId, column: ColumnId) -> CellId {
    let MediaEditResult::MatrixMediaCreated { cell, .. } = edit(
        engine,
        media,
        MatrixEdit::CreateCellMedia {
            layer,
            column,
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    cell
}

#[test]
fn relocation_and_swaps_preserve_actual_subtrees_with_exact_single_step_history_and_reload() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let columns = columns(&mut engine, matrix.media, 3);
    let a = owned(&mut engine, matrix.media, rows[0], columns[0]);
    let b = owned(&mut engine, matrix.media, rows[1], columns[1]);
    // A third Cell occupies a transient address during cross-Layer swaps.
    // Only the completed atomic structure may be validated/published.
    let resident = owned(&mut engine, matrix.media, rows[0], columns[1]);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCell {
            layer: rows[0],
            column: columns[0],
            settings: CellSettings {
                position: [17.0, 23.0],
                opacity: 0.4,
                ..Default::default()
            },
        },
    );
    for (target_layer, target_column, policy) in [
        (rows[0], columns[2], CellOccupiedDrop::Reject),
        (rows[1], columns[1], CellOccupiedDrop::Swap),
        (rows[0], columns[2], CellOccupiedDrop::Swap),
        (rows[1], columns[2], CellOccupiedDrop::Reject),
        (rows[1], columns[1], CellOccupiedDrop::Swap),
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let initial = inspect(&mut engine, matrix.media);
        let original = initial.cells.iter().find(|entry| entry.id == a).unwrap();
        let target = initial
            .cells
            .iter()
            .find(|entry| entry.layer == target_layer && entry.definition.column == target_column);
        let id = engine.node_id_by_uuid(a.0).unwrap();
        let count = engine.nodes.iter().count();
        let history = engine.undo_len();
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::RelocateCell {
                cell: a,
                layer: target_layer,
                column: target_column,
                occupied: policy,
            },
        );
        let next = inspect(&mut engine, matrix.media);
        let moved = next.cells.iter().find(|entry| entry.id == a).unwrap();
        assert_eq!((moved.layer, moved.definition.column), (target_layer, target_column));
        assert_eq!(moved.media, original.media);
        assert_eq!(moved.definition.settings, original.definition.settings);
        assert_eq!(moved.definition.application, original.definition.application);
        assert_eq!(engine.node_id_by_uuid(a.0), Some(id));
        assert_eq!(engine.nodes.iter().count(), count);
        if let Some(target) = target {
            let swapped = next.cells.iter().find(|entry| entry.id == target.id).unwrap();
            assert_eq!(
                (swapped.layer, swapped.definition.column),
                (original.layer, original.definition.column)
            );
            assert_eq!(swapped.media, target.media);
        }
        assert_eq!(engine.undo_len(), history + 1);
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
        let mut loaded = from_sparse_project_json::<AppNode>(&after).unwrap();
        loaded.apply_edits().unwrap();
        crate::matrix::validate_project(&loaded).unwrap();
        // Hydration may place declared Parameters before user children. Compare every
        // exact record/parent and the complete domain ordering, rather than that interleave.
        assert_eq!(
            records(&to_sparse_project_json_pretty(&loaded).unwrap()),
            records(&after)
        );
        assert_eq!(
            serde_json::to_value(inspect(&mut loaded, matrix.media)).unwrap(),
            serde_json::to_value(next).unwrap()
        );
    }
    assert!(engine.node_id_by_uuid(b.0).is_some());
    assert!(engine.node_id_by_uuid(resident.0).is_some());
}

#[test]
fn occupied_prepared_foreign_missing_and_cycle_destinations_reject_without_partial_mutation() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2);
    let columns = columns(&mut engine, matrix.media, 2);
    let a = owned(&mut engine, matrix.media, layer(rows[0]), columns[0]);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::EnsureCell {
            layer: layer(rows[1]),
            column: columns[1],
        },
    );
    let foreign = create(&mut engine, MediaKind::Matrix);
    let foreign_row = layer(layers(&mut engine, foreign.media, None, 1)[0]);
    let foreign_column = super::columns(&mut engine, foreign.media, 1)[0];
    let other = owned(&mut engine, foreign.media, foreign_row, foreign_column);
    for intent in [
        MatrixEdit::RelocateCell {
            cell: CellId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
            layer: layer(rows[0]),
            column: columns[1],
            occupied: CellOccupiedDrop::Swap,
        },
        MatrixEdit::RelocateCell {
            cell: a,
            layer: layer(rows[1]),
            column: columns[1],
            occupied: CellOccupiedDrop::Reject,
        },
        MatrixEdit::RelocateCell {
            cell: other,
            layer: layer(rows[0]),
            column: columns[1],
            occupied: CellOccupiedDrop::Swap,
        },
        MatrixEdit::RelocateCell {
            cell: a,
            layer: foreign_row,
            column: columns[1],
            occupied: CellOccupiedDrop::Swap,
        },
        MatrixEdit::RelocateCell {
            cell: a,
            layer: layer(rows[0]),
            column: foreign_column,
            occupied: CellOccupiedDrop::Swap,
        },
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: intent
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
    // Returning to the same address is an exact no-op, including history.
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RelocateCell {
            cell: a,
            layer: layer(rows[0]),
            column: columns[0],
            occupied: CellOccupiedDrop::Reject,
        },
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let target_output = inspect(&mut engine, matrix.media)
        .items
        .iter()
        .find(|item| item.id == rows[1])
        .unwrap()
        .channel;
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(rows[0]),
                column: columns[0],
                source: Some(target_output),
            }],
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::RelocateCell {
                    cell: a,
                    layer: layer(rows[1]),
                    column: columns[0],
                    occupied: CellOccupiedDrop::Swap
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn relocation_keeps_running_clip_source_launch_and_application_identity_until_an_explicit_launch() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let columns = columns(&mut engine, matrix.media, 2);
    let a = owned(&mut engine, matrix.media, rows[0], columns[0]);
    let b = owned(&mut engine, matrix.media, rows[1], columns[1]);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCellApplication {
            layer: rows[0],
            column: columns[0],
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\never-launched.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[0],
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let time = super::cell_playback::time();
    let captured = super::cell_playback::capture(&engine, matrix.media, rows[0], time);
    assert_eq!(captured.inputs.len(), 1);
    assert_eq!(captured.applications.references.len(), 1);
    let playback = crate::matrix::playback::inspect(&engine, matrix.media).unwrap();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RelocateCell {
            cell: a,
            layer: rows[1],
            column: columns[1],
            occupied: CellOccupiedDrop::Swap,
        },
    );
    let after = super::cell_playback::capture(&engine, matrix.media, rows[0], time);
    assert_eq!(
        after.inputs.iter().map(|input| input.source).collect::<Vec<_>>(),
        captured.inputs.iter().map(|input| input.source).collect::<Vec<_>>()
    );
    assert_eq!(after.applications.references, captured.applications.references);
    assert_eq!(
        serde_json::to_value(crate::matrix::playback::inspect(&engine, matrix.media).unwrap()).unwrap(),
        serde_json::to_value(playback).unwrap()
    );
    let snapshot = inspect(&mut engine, matrix.media);
    assert_eq!(
        snapshot
            .cells
            .iter()
            .find(|cell| cell.layer == rows[0] && cell.definition.column == columns[0])
            .unwrap()
            .id,
        b
    );
    assert_eq!(
        snapshot
            .cells
            .iter()
            .find(|cell| cell.layer == rows[1] && cell.definition.column == columns[1])
            .unwrap()
            .id,
        a
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[1],
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    let moved = super::cell_playback::capture(&engine, matrix.media, rows[1], time);
    assert_eq!(moved.inputs.len(), 1);
    assert_eq!(moved.inputs[0].source, captured.inputs[0].source);
    assert_eq!(moved.applications.references, captured.applications.references);
}

#[test]
fn same_column_reparent_refreshes_application_membership_for_the_next_launch() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let column = columns(&mut engine, matrix.media, 1)[0];
    let cell = owned(&mut engine, matrix.media, rows[0], column);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCellApplication {
            layer: rows[0],
            column,
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\never-launched.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[0],
            column,
            mode: LaunchMode::Cut,
        },
    );
    let time = super::cell_playback::time();
    let before = super::cell_playback::capture(&engine, matrix.media, rows[0], time);
    assert_eq!(before.applications.references.len(), 1);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RelocateCell {
            cell,
            layer: rows[1],
            column,
            occupied: CellOccupiedDrop::Reject,
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[1],
            column,
            mode: LaunchMode::Cut,
        },
    );
    let after = super::cell_playback::capture(&engine, matrix.media, rows[1], time);
    assert_eq!(after.inputs.len(), 1);
    assert_eq!(after.inputs[0].source, before.inputs[0].source);
    assert_eq!(after.applications.references, before.applications.references);
    for (redo, row) in [(false, rows[0]), (true, rows[1])] {
        assert!(if redo {
            engine.redo().unwrap()
        } else {
            engine.undo().unwrap()
        });
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::LaunchCell {
                layer: row,
                column,
                mode: LaunchMode::Cut,
            },
        );
        let replayed = super::cell_playback::capture(&engine, matrix.media, row, time);
        assert_eq!(replayed.inputs.len(), 1);
        assert_eq!(replayed.inputs[0].source, before.inputs[0].source);
        assert_eq!(replayed.applications.references, before.applications.references);
    }
}
