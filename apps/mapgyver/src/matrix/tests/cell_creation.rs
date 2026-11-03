use super::*;

#[test]
fn contextual_cell_media_creation_uses_shared_defaults_preserves_settings_and_is_atomic_through_history_copy_project() {
    for choice in crate::media::creation::media_choices() {
        let mut engine = fixture();
        let matrix = create(&mut engine, MediaKind::Matrix);
        let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
        let column = columns(&mut engine, matrix.media, 1)[0];
        let settings = CellSettings {
            position: [2.0, -3.0],
            opacity: 0.25,
            enabled: false,
        };
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::ConfigureCell {
                layer: row,
                column,
                settings,
            },
        );
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let MediaEditResult::MatrixMediaCreated { cell, media } = edit(
            &mut engine,
            matrix.media,
            MatrixEdit::CreateCellMedia {
                layer: row,
                column,
                media_kind: choice.media_kind,
            },
        ) else {
            panic!()
        };
        assert_eq!(engine.undo_len(), history + 1);
        let snapshot = inspect(&mut engine, matrix.media);
        assert_eq!(snapshot.cells[0].id, cell);
        assert_eq!(snapshot.cells[0].media, Some(media));
        assert_eq!(snapshot.cells[0].definition.settings, settings);
        assert_eq!(
            crate::media::authoring::media(&engine, media).unwrap().1,
            choice.media_kind
        );
        if choice.media_kind == MediaKind::Matrix {
            let nested = inspect(&mut engine, media);
            assert_eq!(nested.items.len(), 3);
            assert_eq!(nested.columns.len(), 3);
            assert!(nested.cells.is_empty());
        }
        crate::media::validate_project(&engine).unwrap();
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        assert_eq!(inspect(&mut loaded, matrix.media), snapshot);
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        let ack = invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::CreateCellMedia {
                    layer: row,
                    column,
                    media_kind: choice.media_kind,
                },
            },
        );
        assert!(!ack.success);
        assert_eq!(engine.undo_len(), history + 1);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        edit(&mut engine, matrix.media, MatrixEdit::DuplicateColumn { column });
        let copied = inspect(&mut engine, matrix.media);
        assert_eq!(copied.cells.len(), 2);
        assert!(copied.cells.iter().all(|cell| cell.definition.settings == settings));
        assert_ne!(copied.cells[0].media, copied.cells[1].media);
        assert!(matches!(
            engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()),
            Some(AppNode::Media(_) | AppNode::ReferenceMedia(_))
        ));
    }
}

#[test]
fn sparse_cell_creation_does_not_activate_a_playing_empty_address_and_rejects_foreign_addresses() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let other = create(&mut engine, MediaKind::Matrix);
    let foreign = columns(&mut engine, other.media, 1)[0];
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let rejected = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::CreateCellMedia {
                layer: row,
                column: foreign,
                media_kind: MediaKind::SolidColor,
            },
        },
    );
    assert!(!rejected.success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let time = super::cell_playback::time();
    assert!(
        super::cell_playback::capture(&engine, matrix.media, row, time)
            .inputs
            .is_empty()
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::CreateCellMedia {
            layer: row,
            column,
            media_kind: MediaKind::SolidColor,
        },
    );
    assert!(
        super::cell_playback::capture(&engine, matrix.media, row, time)
            .inputs
            .is_empty()
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: row,
            column,
            mode: LaunchMode::Cut,
        },
    );
    assert_eq!(
        super::cell_playback::capture(&engine, matrix.media, row, time)
            .inputs
            .len(),
        1
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}
