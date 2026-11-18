use super::{
    column_enabled::enabled,
    column_groups::{group as add_group, nested_columns},
    controls::{current, parameter, pulse},
    *,
};

#[test]
fn column_group_transport_resolves_direct_children_after_reorder() {
    use MatrixNavigationDirection::{Next, Previous};
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let foreign = create(&mut engine, MediaKind::Matrix).media;
    let outside = columns(&mut engine, media, 1)[0];
    let group = add_group(&mut engine, media, None);
    let first = nested_columns(&mut engine, media, group, 1)[0];
    let empty = add_group(&mut engine, media, Some(group));
    let nested = add_group(&mut engine, media, Some(group));
    let nested_first = nested_columns(&mut engine, media, nested, 1)[0];
    let deep = add_group(&mut engine, media, Some(nested));
    let nested_last = nested_columns(&mut engine, media, deep, 1)[0];
    let tail = nested_columns(&mut engine, media, group, 1)[0];
    let disabled = nested_columns(&mut engine, media, group, 1)[0];
    enabled(&mut engine, disabled.0, false);
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: [nested_last, tail]
                .map(|column| CellAssignment {
                    layer: row,
                    column,
                    source: Some(source),
                })
                .into(),
        },
    );
    let history = engine.undo_len();

    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: outside,
            mode: LaunchMode::Cut,
        },
    );
    pulse(&mut engine, group.0, "play");
    assert_eq!(current(&engine, media).desired_column, Some(first));
    pulse(&mut engine, group.0, "next_child");
    assert_eq!(current(&engine, media).desired_column, Some(nested_first));
    pulse(&mut engine, nested.0, "next_child");
    assert_eq!(current(&engine, media).desired_column, Some(nested_last));
    let MediaEditResult::MatrixControlInvoked {
        owner,
        parameter: trigger,
    } = edit(
        &mut engine,
        media,
        MatrixEdit::NavigateGroupChild { group, direction: Next },
    )
    else {
        panic!("Group trigger receipt")
    };
    assert_eq!(owner, group.0);
    assert_eq!(
        engine.node_id_by_uuid(trigger),
        Some(parameter(&engine, group.0, "next_child"))
    );
    let at_tail = current(&engine, media);
    assert_eq!(at_tail.desired_column, Some(tail));
    assert_eq!(at_tail.transition.duration_ms, 250);
    pulse(&mut engine, group.0, "next_child");
    pulse(&mut engine, empty.0, "play");
    pulse(&mut engine, empty.0, "previous_child");
    assert_eq!(current(&engine, media), at_tail);
    edit(
        &mut engine,
        media,
        MatrixEdit::NavigateGroupChild {
            group,
            direction: Previous,
        },
    );
    assert_eq!(current(&engine, media).desired_column, Some(nested_first));
    assert_eq!(engine.undo_len(), history);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: foreign,
                edit: MatrixEdit::PlayColumnGroup { group }
            }
        )
        .success
    );

    edit(
        &mut engine,
        media,
        MatrixEdit::MoveColumnItem {
            item: ColumnItemId::Column { id: tail },
            parent: Some(group),
            after: None,
        },
    );
    edit(&mut engine, media, MatrixEdit::PlayColumnGroup { group });
    assert_eq!(current(&engine, media).desired_column, Some(tail));
    pulse(&mut engine, group.0, "next_child");
    assert_eq!(current(&engine, media).desired_column, Some(first));
    assert_eq!(engine.undo_len(), history + 1);
}
