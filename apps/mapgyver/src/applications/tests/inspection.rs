use super::*;
#[test]
fn inspection_after_owner_deletion_reports_absence_without_authoring_and_recovers_with_history() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let before = super::super::inspect(&engine, application).unwrap();
    remove(&mut engine, application);
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        matches!(app_edit(&mut engine, ApplicationEdit::Inspect { application }), MediaEditResult::ApplicationMissing { application: missing } if missing == application)
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), document);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application,
                    definition: before.definition.clone()
                }
            }
        )
        .success
    );
    assert!(engine.undo().unwrap());
    let restored = super::super::inspect(&engine, application).unwrap();
    assert_eq!(restored.id, before.id);
    assert_eq!(restored.definition, before.definition);
    assert_eq!(restored.definition_instance, before.definition_instance);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(super::super::inspect(&loaded, application).unwrap().id, application);
    assert!(engine.redo().unwrap());
    assert!(super::super::inspect(&engine, application).is_err());
}
