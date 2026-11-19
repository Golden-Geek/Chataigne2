use super::*;
use crate::applications::protocol::{ApplicationDefinition, ApplicationId};
use golden_assets::AbsoluteAssetPath;
use golden_core::node::Node;
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};

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
    let file_drop = |name, column: Option<ColumnId>| MatrixEdit::DropFile {
        path: AbsoluteAssetPath::try_from(directory.path().join(name).to_string_lossy().into_owned()).unwrap(),
        target: column.map_or(MatrixFileTarget::NewColumn { layer }, |column| MatrixFileTarget::Cell {
            layer,
            column,
        }),
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

#[test]
fn executable_drops_target_independent_controls_and_keep_existing_settings() {
    let mut engine = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = |name| AbsoluteAssetPath::try_from(dir.path().join(name).to_string_lossy().into_owned()).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let layer = layer(layers(&mut engine, matrix, None, 1)[0]);
    let column = columns(&mut engine, matrix, 1)[0];
    let MediaEditResult::MatrixColumnGroupCreated { group } = edit(
        &mut engine,
        matrix,
        MatrixEdit::AddColumnGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!("expected new Column Group");
    };
    let column_target = MatrixFileTarget::ColumnItem {
        item: ColumnItemId::Column { id: column },
    };
    let group_target = MatrixFileTarget::ColumnItem {
        item: ColumnItemId::Group { id: group },
    };
    let cell_target = MatrixFileTarget::Cell { layer, column };
    let existing = ApplicationDefinition {
        executable: Some(path("old.exe").as_str().into()),
        arguments: vec!["keep this argument".into()],
        environment: Some(vec![("MAPGYVER_TEST".into(), "keep this value".into())]),
        ..Default::default()
    };
    edit(
        &mut engine,
        matrix,
        MatrixEdit::ConfigureColumnApplication {
            column,
            definition: existing.clone(),
            pre: 2,
            post: 3,
        },
    );
    let column_app = ApplicationId(column.0);
    let control = crate::applications::control_node(&engine, column_app).unwrap().id();
    let inherit_environment = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().meta.decl_id.0 == "parameters/inherit_environment"
                && node
                    .node_data()
                    .parent
                    .and_then(|parent| engine.nodes.get(parent))
                    .and_then(|folder| folder.node_data().parent)
                    == Some(control)
        })
        .unwrap()
        .0;
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: inherit_environment,
            value: ParamValue::Bool(true),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(
        crate::applications::inspect(&engine, column_app)
            .unwrap()
            .definition
            .environment,
        None
    );
    let file_drop = |target, name| MatrixEdit::DropFile {
        target,
        path: path(name),
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(
        before.contains("keep this value"),
        "inherit mode keeps dormant environment entries"
    );
    let history = engine.undo_len();
    for (target, name) in [
        (column_target, "not an executable.png"),
        (MatrixFileTarget::NewColumn { layer }, "invalid here.exe"),
    ] {
        let command = MediaEdit::Matrix {
            media: matrix,
            edit: file_drop(target, name),
        };
        assert!(!invoke(&mut engine, command).success);
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);

    edit(&mut engine, matrix, file_drop(column_target, "new.EXE"));
    let new_file = path("new.EXE").as_str().to_owned();
    let after_column = to_sparse_project_json_pretty(&engine).unwrap();
    assert_eq!(
        after_column,
        before.replace(
            &serde_json::to_string(&existing.executable).unwrap(),
            &serde_json::to_string(&new_file).unwrap(),
        )
    );
    assert_eq!(engine.undo_len(), history + 1);

    edit(&mut engine, matrix, file_drop(group_target, "group.exe"));
    let MediaEditResult::MatrixCellPrepared { cell, application } =
        edit(&mut engine, matrix, file_drop(cell_target, "cell.exe"))
    else {
        panic!("expected a sparse Cell with App Control");
    };
    assert_eq!(application, ApplicationId(cell.0));
    let authored = inspect(&mut engine, matrix);
    assert_eq!(authored.cells.len(), 1);
    assert_eq!(authored.cells[0].media, None);
    assert_eq!(engine.undo_len(), history + 3);
    assert!(engine.undo().unwrap());
    assert!(inspect(&mut engine, matrix).cells.is_empty());
    assert!(engine.redo().unwrap());

    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    for (application, expected) in [
        (column_app, new_file),
        (ApplicationId(group.0), path("group.exe").as_str().into()),
        (application, path("cell.exe").as_str().into()),
    ] {
        let definition = crate::applications::inspect(&loaded, application).unwrap().definition;
        assert_eq!(definition.executable, Some(expected));
    }
}
