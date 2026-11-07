use super::*;

#[test]
fn repeating_a_source_assignment_preserves_owned_reference_identity_and_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let item = layers(&mut engine, matrix.media, None, 1)[0];
    let column = columns(&mut engine, matrix.media, 1)[0];
    let assignment = CellAssignment {
        layer: layer(item),
        column,
        source: Some(source),
    };
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![assignment.clone()],
        },
    );
    let before = inspect(&mut engine, matrix.media);
    let history = engine.undo_len();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![assignment],
        },
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(inspect(&mut engine, matrix.media), before);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let owned = before.cells[0].definition.source.unwrap();
    assert_ne!(owned.channel, source.channel);
    assert!(
        matches!(crate::media::channel(&engine, owned).unwrap().definition.source,
        ChannelSource::Reference { target: Some(target) } if target == source)
    );
}
