use super::{
    column_enabled::enabled,
    column_groups::{group, nested_columns},
    controls::{current, pulse},
    *,
};

#[test]
fn layer_navigation_steps_only_playable_cells_without_changing_other_playback() {
    use MatrixNavigationDirection::{Next, Previous};

    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let peer_media = create(&mut engine, MediaKind::Matrix).media;
    let rows = layers(&mut engine, media, None, 3);
    let (row, other, empty_row) = (layer(rows[0]), layer(rows[1]), layer(rows[2]));
    let regular = columns(&mut engine, media, 5);
    let (first, empty, disabled_cell, disabled_column) = (regular[0], regular[2], regular[3], regular[4]);
    let outer = group(&mut engine, media, None);
    let nested = group(&mut engine, media, Some(outer));
    let last = nested_columns(&mut engine, media, nested, 1)[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    let assignments = [first, disabled_cell, disabled_column, last]
        .map(|column| CellAssignment {
            layer: row,
            column,
            source: Some(source),
        })
        .into_iter()
        .chain([CellAssignment {
            layer: other,
            column: last,
            source: Some(source),
        }])
        .collect();
    edit(&mut engine, media, MatrixEdit::SetCells { assignments });
    edit(
        &mut engine,
        media,
        MatrixEdit::EnsureCell {
            layer: row,
            column: empty,
        },
    );
    let disabled = inspect(&mut engine, media)
        .cells
        .into_iter()
        .find(|cell| cell.layer == row && cell.definition.column == disabled_cell)
        .unwrap()
        .id;
    enabled(&mut engine, disabled.0, false);
    enabled(&mut engine, disabled_column.0, false);
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();

    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: first,
            mode: LaunchMode::Cut,
        },
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: other,
            column: last,
            mode: LaunchMode::Cut,
        },
    );
    let before = current(&engine, media);
    let other_playback = before.layers.iter().find(|request| request.layer == other).unwrap();
    pulse(&mut engine, row.0, "next_cell");
    let after = current(&engine, media);
    assert_eq!(after.desired_column, before.desired_column);
    assert_eq!(after.launch_id, before.launch_id);
    assert_eq!(
        after.layers.iter().find(|request| request.layer == other),
        Some(other_playback)
    );
    let stepped = after.layers.iter().find(|request| request.layer == row).unwrap();
    assert_eq!(stepped.column, Some(last));
    assert!(stepped.individual);
    assert_eq!(stepped.transition.duration_ms, 250);
    edit(
        &mut engine,
        media,
        MatrixEdit::NavigateCell {
            layer: row,
            direction: Next,
        },
    );
    pulse(&mut engine, empty_row.0, "next_cell");
    assert_eq!(current(&engine, media), after);

    let MediaEditResult::MatrixControlInvoked { owner, parameter } = edit(
        &mut engine,
        media,
        MatrixEdit::NavigateCell {
            layer: row,
            direction: Previous,
        },
    ) else {
        panic!("Layer trigger receipt")
    };
    assert_eq!(owner, row.0);
    assert_eq!(
        engine.node_id_by_uuid(parameter),
        Some(super::controls::parameter(&engine, row.0, "previous_cell"))
    );
    assert_eq!(
        current(&engine, media)
            .layers
            .iter()
            .find(|request| request.layer == row)
            .and_then(|request| request.column),
        Some(first)
    );

    pulse(&mut engine, row.0, "stop");
    assert_eq!(
        current(&engine, media)
            .layers
            .iter()
            .find(|request| request.layer == row)
            .unwrap()
            .column,
        None
    );
    pulse(&mut engine, row.0, "next_cell");
    let restarted = current(&engine, media);
    assert_eq!(restarted.desired_column, Some(first));
    assert_eq!(
        restarted
            .layers
            .iter()
            .find(|request| request.layer == row)
            .unwrap()
            .column,
        Some(first)
    );
    for (matrix, layer) in [(peer_media, row), (media, LayerId(outer.0))] {
        let denied = invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix,
                edit: MatrixEdit::NavigateCell { layer, direction: Next },
            },
        );
        assert!(!denied.success);
    }
    assert_eq!(current(&engine, media), restarted);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
}
