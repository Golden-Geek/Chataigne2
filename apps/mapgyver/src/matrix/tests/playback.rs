use super::*;

#[test]
fn zero_duration_configured_launch_uses_column_override_and_rejects_foreign_columns() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let other = create(&mut engine, MediaKind::Matrix);
    let own = columns(&mut engine, matrix.media, 1)[0];
    let foreign = columns(&mut engine, other.media, 1)[0];
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureColumn {
            column: own,
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms: 0,
                    curve: TransitionCurve::SmoothStep,
                }),
            },
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: own,
            mode: LaunchMode::Configured,
        },
    );
    let committed = current(&mut engine, matrix.media);
    assert_eq!(committed.desired_column, Some(own));
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::LaunchColumn {
                    column: foreign,
                    mode: LaunchMode::Cut
                }
            }
        )
        .success
    );
    assert_eq!(current(&mut engine, matrix.media), committed);
}

fn current(engine: &mut AppEngine, media: MediaId) -> MatrixPlaybackSnapshot {
    let MediaEditResult::MatrixPlayback { snapshot } = super::super::playback::inspect(engine, media).unwrap() else {
        panic!()
    };
    snapshot
}

#[test]
fn cut_bursts_and_configured_launches_are_ephemeral_without_authored_history() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let columns = columns(&mut engine, matrix.media, 2);
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert_eq!(current(&mut engine, matrix.media), MatrixPlaybackSnapshot::default());
    for index in 0..1000 {
        let column = columns[index % 2];
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::LaunchColumn {
                column,
                mode: LaunchMode::Cut,
            },
        );
        assert_eq!(current(&mut engine, matrix.media).desired_column, Some(column));
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::LaunchColumn {
                column: columns[0],
                mode: LaunchMode::Configured,
            },
        },
    );
    assert!(ack.success);
    assert_eq!(current(&mut engine, matrix.media).transition.duration_ms, 250);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    let mut restored = from_sparse_project_json::<AppNode>(&authored).unwrap();
    assert_eq!(current(&mut restored, matrix.media), MatrixPlaybackSnapshot::default());
}

#[test]
fn queued_launch_is_invisible_and_discard_preserves_previous_committed_launch() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let columns = columns(&mut engine, matrix.media, 2);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let committed = current(&mut engine, matrix.media);
    super::super::playback::launch(&mut engine, matrix.media, columns[1], LaunchMode::Cut).unwrap();
    assert_eq!(current(&mut engine, matrix.media), committed);
    engine.edits.pending.clear();
    assert_eq!(current(&mut engine, matrix.media), committed);
    super::super::playback::launch(&mut engine, matrix.media, columns[1], LaunchMode::Cut).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(current(&mut engine, matrix.media).desired_column, Some(columns[1]));
}
