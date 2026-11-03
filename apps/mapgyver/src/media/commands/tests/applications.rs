use super::*;
use crate::applications::protocol::*;
use crate::matrix::protocol::*;

#[test]
fn app_file_command_preserves_other_typed_settings_and_has_atomic_rejection_and_history() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } = apply(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumns { count: 1 },
        },
    ) else {
        panic!()
    };
    let application = ApplicationId(columns[0].0);
    let definition = ApplicationDefinition {
        arguments: vec!["two words".into()],
        environment: Some(vec![("APP_TEST".into(), "literal value".into())]),
        ..Default::default()
    };
    apply(
        &mut engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Configure {
                application,
                definition: definition.clone(),
            },
        },
    );
    let command = command(&mut engine, MediaCommandKind::SetApplicationFile);
    set(&mut engine, command, "application_owner", reference(application.0));
    let executable = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    set(
        &mut engine,
        command,
        "application_file",
        ParamValue::File(executable.clone()),
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    apply(&mut engine, MediaEdit::ExecuteCommand { command });
    let mut expected = definition.clone();
    expected.executable = Some(executable);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        expected
    );
    assert_eq!(engine.undo_len(), history + 1);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        crate::applications::inspect(&loaded, application).unwrap().definition,
        expected
    );
    set(
        &mut engine,
        command,
        "application_file",
        ParamValue::File("relative.exe".into()),
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(!invoke(&mut engine, MediaEdit::ExecuteCommand { command }).success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
    set(
        &mut engine,
        command,
        "application_file",
        ParamValue::File(String::new()),
    );
    apply(&mut engine, MediaEdit::ExecuteCommand { command });
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
}
