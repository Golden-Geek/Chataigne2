use super::*;
use golden_assets::AbsoluteAssetPath;

#[test]
fn imports_replace_only_the_cell_media_and_create_columns_in_the_same_history_step() {
    let mut engine = fixture();
    let directory = tempfile::tempdir().unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let layer = layer(layers(&mut engine, matrix, None, 1)[0]);
    let column = columns(&mut engine, matrix, 1)[0];
    let settings = CellSettings {
        opacity: 0.4,
        position: [12.0, 70.0],
        ..Default::default()
    };
    edit(
        &mut engine,
        matrix,
        MatrixEdit::ConfigureCell {
            layer,
            column,
            settings: settings.clone(),
        },
    );
    let initial = inspect(&mut engine, matrix);
    let history = engine.undo_len();
    let file_drop = |name, column| MatrixEdit::DropMediaFile {
        path: AbsoluteAssetPath::try_from(directory.path().join(name).to_string_lossy().into_owned()).unwrap(),
        layer,
        column,
    };
    let unsupported = MediaEdit::Matrix {
        media: matrix,
        edit: file_drop("unsupported.txt", None),
    };
    assert!(!invoke(&mut engine, unsupported).success);
    assert_eq!(inspect(&mut engine, matrix), initial);
    assert_eq!(engine.undo_len(), history);

    let MediaEditResult::MatrixFileImported { cell, media, .. } =
        edit(&mut engine, matrix, file_drop("Picture.WEBP", Some(column)))
    else {
        panic!("expected imported Media");
    };
    assert_eq!(cell, initial.cells[0].id);
    let first = inspect(&mut engine, matrix);
    assert_eq!(first.cells[0].definition.settings, settings);
    assert_eq!(first.cells[0].media, Some(media));

    let MediaEditResult::MatrixFileImported {
        cell: replaced,
        media: replacement,
        ..
    } = edit(&mut engine, matrix, file_drop("Replacement.jpg", Some(column)))
    else {
        panic!("expected replaced Media");
    };
    assert_eq!(replaced, cell);
    assert_ne!(replacement, media);
    assert_eq!(engine.undo_len(), history + 2);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix), first);
    assert!(engine.redo().unwrap());
    let after_replace = inspect(&mut engine, matrix);
    assert_eq!(after_replace.cells[0].definition.settings, settings);
    assert_eq!(
        after_replace.cells[0].definition.application,
        first.cells[0].definition.application
    );

    let MediaEditResult::MatrixFileImported { column: added, .. } =
        edit(&mut engine, matrix, file_drop("Other.png", None))
    else {
        panic!("expected new Column");
    };
    let saved = inspect(&mut engine, matrix);
    assert_eq!(saved.columns[1].id, added);
    assert_eq!(saved.columns[1].label, "Column 2");
    assert_eq!(saved.cells.len(), 2);
    assert_eq!(engine.undo_len(), history + 3);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix), after_replace);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, matrix), saved);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded, matrix), saved);
}
