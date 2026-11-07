use super::{
    cell_duplicates::{application_at, duplicate, owned, scene},
    *,
};

fn view(engine: &mut AppEngine, media: MediaId, cell: CellId) -> MatrixViewCell {
    let MediaEditResult::MatrixViewport { snapshot } = edit(
        engine,
        media,
        MatrixEdit::InspectViewport {
            selection: None,
            viewport: MatrixViewport {
                row_start: 0,
                row_count: 16,
                axis_start: 0,
                axis_count: 8,
            },
        },
    ) else {
        panic!()
    };
    snapshot.cells.into_iter().find(|item| item.cell.id == cell).unwrap()
}

fn assert_view(actual: MatrixViewCell, expected: &MatrixViewCell) {
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

fn configure_application(engine: &mut AppEngine, media: MediaId, row: LayerId, column: ColumnId) {
    edit(
        engine,
        media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column,
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\lifetime-producer.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    );
}

#[test]
fn cross_matrix_copies_survive_source_removal_and_references_recover_exact_targets_through_history_and_reload() {
    for removal in ["content", "layer", "matrix"] {
        let mut engine = fixture();
        let (source_matrix, source_row, source_columns) = scene(&mut engine);
        let (target, row, columns) = scene(&mut engine);
        let source = owned(&mut engine, source_matrix, source_row, source_columns[0]);
        configure_application(&mut engine, source_matrix, source_row, source_columns[0]);
        let copy = duplicate(&mut engine, target, source, row, columns[0], CellTransferAction::Copy);
        let reference = duplicate(
            &mut engine,
            target,
            source,
            row,
            columns[1],
            CellTransferAction::Reference,
        );
        let original = view(&mut engine, target, reference);
        let independent = view(&mut engine, target, copy);
        assert!(!original.source_missing);
        assert_eq!(application_at(&engine, target, row, columns[1]).unwrap().0, source.0);
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        match removal {
            "content" => {
                edit(
                    &mut engine,
                    source_matrix,
                    MatrixEdit::SetCells {
                        assignments: vec![CellAssignment {
                            layer: source_row,
                            column: source_columns[0],
                            source: None,
                        }],
                    },
                );
            }
            "layer" => {
                edit(
                    &mut engine,
                    source_matrix,
                    MatrixEdit::RemoveItem {
                        item: MatrixItemId::Layer { id: source_row },
                    },
                );
            }
            "matrix" => {
                apply(&mut engine, MediaEdit::RemoveMedia { media: source_matrix });
            }
            _ => unreachable!(),
        }
        assert_eq!(engine.undo_len(), history + 1);
        let missing = view(&mut engine, target, reference);
        assert!(missing.source_missing, "{removal}");
        assert!(missing.source_preview.is_none());
        assert_eq!(missing.cell.id, reference);
        assert_eq!(missing.cell.media, original.cell.media);
        assert_eq!(missing.cell.definition.application.0, source.0);
        let local_application = application_at(&engine, target, row, columns[1]);
        assert_eq!(local_application.is_some(), removal == "content", "{removal}");
        assert_view(view(&mut engine, target, copy), &independent);
        assert_eq!(application_at(&engine, target, row, columns[0]).unwrap().0, copy.0);
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&after).unwrap();
        loaded.apply_edits().unwrap();
        assert_view(view(&mut loaded, target, reference), &missing);
        assert_view(view(&mut loaded, target, copy), &independent);
        assert_eq!(application_at(&loaded, target, row, columns[1]), local_application);
        assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_view(view(&mut engine, target, reference), &original);
        assert_eq!(application_at(&engine, target, row, columns[1]).unwrap().0, source.0);
        assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
        assert_view(view(&mut engine, target, reference), &missing);
        assert_eq!(application_at(&engine, target, row, columns[1]), local_application);
    }
}

#[test]
fn replaced_source_does_not_retarget_references_and_empty_cell_duplicates_keep_independent_authored_settings() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let source = owned(&mut engine, media, row, columns[0]);
    let copy = duplicate(&mut engine, media, source, row, columns[1], CellTransferAction::Copy);
    let reference = duplicate(
        &mut engine,
        media,
        source,
        row,
        columns[2],
        CellTransferAction::Reference,
    );
    let original = view(&mut engine, media, reference);
    let copied = view(&mut engine, media, copy);
    let replacement = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: row,
                column: columns[0],
                source: Some(replacement),
            }],
        },
    );
    let missing = view(&mut engine, media, reference);
    assert!(missing.source_missing);
    assert_eq!(missing.cell.media, original.cell.media);
    assert_view(view(&mut engine, media, copy), &copied);
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert_view(view(&mut engine, media, reference), &original);

    let (empty_media, empty_row, empty_columns) = scene(&mut engine);
    let settings = CellSettings {
        position: [47.0, -19.0],
        opacity: 0.3,
        ..Default::default()
    };
    edit(
        &mut engine,
        empty_media,
        MatrixEdit::ConfigureCell {
            layer: empty_row,
            column: empty_columns[0],
            settings,
        },
    );
    configure_application(&mut engine, empty_media, empty_row, empty_columns[0]);
    let empty = inspect(&mut engine, empty_media).cells[0].id;
    let empty_copy = duplicate(
        &mut engine,
        empty_media,
        empty,
        empty_row,
        empty_columns[1],
        CellTransferAction::Copy,
    );
    let empty_reference = duplicate(
        &mut engine,
        empty_media,
        empty,
        empty_row,
        empty_columns[2],
        CellTransferAction::Reference,
    );
    for (cell, owner) in [(empty_copy, empty_copy), (empty_reference, empty)] {
        let current = view(&mut engine, empty_media, cell);
        assert_eq!(current.cell.definition.settings, settings);
        assert!(current.cell.media.is_none());
        assert!(!current.source_missing);
        assert!(current.source_preview.is_none());
        assert_eq!(current.cell.definition.application.0, owner.0);
    }
    edit(
        &mut engine,
        empty_media,
        MatrixEdit::ConfigureCell {
            layer: empty_row,
            column: empty_columns[0],
            settings: CellSettings::default(),
        },
    );
    assert_eq!(
        view(&mut engine, empty_media, empty_copy).cell.definition.settings,
        settings
    );
    assert_eq!(
        view(&mut engine, empty_media, empty_reference).cell.definition.settings,
        settings
    );
    let content = owned(&mut engine, empty_media, empty_row, empty_columns[2]);
    assert_eq!(content, empty_reference);
    let independent = view(&mut engine, empty_media, empty_reference);
    assert!(independent.cell.media.is_some());
    assert_eq!(independent.cell.definition.application.0, empty_reference.0);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_view(view(&mut loaded, empty_media, empty_reference), &independent);
    assert_eq!(
        view(&mut loaded, empty_media, empty_copy).cell.definition.settings,
        settings
    );
}
