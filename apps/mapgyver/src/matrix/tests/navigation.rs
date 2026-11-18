use super::{
    cell_playback::{at, capture, time},
    column_enabled::enabled,
    column_groups::{group, nested_columns},
    controls::{current, pulse},
    *,
};
use MatrixNavigationDirection::{Next, Previous};

fn cut(engine: &mut AppEngine, media: MediaId, column: ColumnId) {
    edit(
        engine,
        media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
}

#[test]
fn matrix_column_navigation_uses_enabled_flattened_order_and_authored_transition() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let first = columns(&mut engine, media, 1)[0];
    let outer = group(&mut engine, media, None);
    let direct = nested_columns(&mut engine, media, outer, 1)[0];
    let inner = group(&mut engine, media, Some(outer));
    let nested = nested_columns(&mut engine, media, inner, 2);
    let last = columns(&mut engine, media, 1)[0];
    enabled(&mut engine, nested[0].0, false);
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumn {
            column: nested[1],
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms: 700,
                    curve: TransitionCurve::Smooth,
                }),
            },
        },
    );
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: [first, last]
                .map(|column| CellAssignment {
                    layer: row,
                    column,
                    source: Some(source),
                })
                .into(),
        },
    );
    let first_cell = inspect(&mut engine, media)
        .cells
        .into_iter()
        .find(|cell| cell.layer == row && cell.definition.column == first)
        .unwrap()
        .definition
        .source
        .unwrap();
    let now = time();
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();

    pulse(&mut engine, media.0, "previous_column");
    let tail = current(&engine, media);
    assert_eq!(tail.desired_column, Some(last));
    edit(&mut engine, media, MatrixEdit::NavigateColumn { direction: Next });
    assert_eq!(current(&engine, media), tail);

    edit(&mut engine, media, MatrixEdit::NavigateColumn { direction: Previous });
    let inner = current(&engine, media);
    assert_eq!(inner.desired_column, Some(nested[1]));
    assert_eq!(inner.transition.duration_ms, 700);
    pulse(&mut engine, media.0, "previous_column");
    assert_eq!(current(&engine, media).desired_column, Some(direct));
    edit(&mut engine, media, MatrixEdit::NavigateColumn { direction: Previous });
    let head = current(&engine, media);
    assert_eq!(head.desired_column, Some(first));
    // The Matrix anchor carries its inherited Layer requests.
    assert!(head.layers.is_empty());
    assert!(
        capture(&engine, media, row, now)
            .inputs
            .iter()
            .any(|input| input.source == first_cell)
    );
    pulse(&mut engine, media.0, "previous_column");
    assert_eq!(current(&engine, media), head);
    pulse(&mut engine, media.0, "next_column");
    assert_eq!(current(&engine, media).desired_column, Some(direct));

    edit(&mut engine, media, MatrixEdit::ClearPlayback);
    let stopped = current(&engine, media);
    assert_eq!(stopped.desired_column, None);
    assert!(stopped.layers.is_empty());
    edit(&mut engine, media, MatrixEdit::NavigateColumn { direction: Next });
    assert_eq!(current(&engine, media).desired_column, Some(first));
    pulse(&mut engine, media.0, "stop_all");
    let stopped = current(&engine, media);
    assert_eq!(stopped.desired_column, None);
    assert!(stopped.layers.is_empty());
    assert!(capture(&engine, media, row, at(now, 1000)).inputs.is_empty());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
}

#[test]
fn matrix_group_navigation_skips_nested_groups_as_targets_and_uses_root_axis_position() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let before = columns(&mut engine, media, 1)[0];
    let outer = group(&mut engine, media, None);
    let disabled = nested_columns(&mut engine, media, outer, 1)[0];
    enabled(&mut engine, disabled.0, false);
    let inner = group(&mut engine, media, Some(outer));
    let deep = nested_columns(&mut engine, media, inner, 2);
    group(&mut engine, media, None); // Empty groups do not consume a navigation step.
    let between = columns(&mut engine, media, 1)[0];
    let second = group(&mut engine, media, None);
    let second_first = nested_columns(&mut engine, media, second, 2)[0];
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();

    pulse(&mut engine, media.0, "next_column_group");
    assert_eq!(current(&engine, media).desired_column, Some(deep[0]));
    cut(&mut engine, media, deep[1]);
    edit(&mut engine, media, MatrixEdit::NavigateColumnGroup { direction: Next });
    let tail = current(&engine, media);
    assert_eq!(tail.desired_column, Some(second_first));
    pulse(&mut engine, media.0, "next_column_group");
    assert_eq!(current(&engine, media), tail);
    pulse(&mut engine, media.0, "previous_column_group");
    assert_eq!(current(&engine, media).desired_column, Some(deep[0]));

    cut(&mut engine, media, between);
    pulse(&mut engine, media.0, "next_column_group");
    assert_eq!(current(&engine, media).desired_column, Some(second_first));
    cut(&mut engine, media, between);
    edit(
        &mut engine,
        media,
        MatrixEdit::NavigateColumnGroup { direction: Previous },
    );
    assert_eq!(current(&engine, media).desired_column, Some(deep[0]));

    cut(&mut engine, media, before);
    let start = current(&engine, media);
    pulse(&mut engine, media.0, "previous_column_group");
    assert_eq!(current(&engine, media), start);
    pulse(&mut engine, media.0, "stop_all");
    edit(
        &mut engine,
        media,
        MatrixEdit::NavigateColumnGroup { direction: Previous },
    );
    assert_eq!(current(&engine, media).desired_column, Some(second_first));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
}
