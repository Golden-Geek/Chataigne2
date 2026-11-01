use super::*;
use golden_core::node::Node;

#[test]
fn inspection_after_deletion_reports_absence_without_authoring_and_recovers_through_history() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let MediaEditResult::Application { snapshot: original } =
        app_edit(&mut engine, ApplicationEdit::Inspect { application })
    else {
        panic!()
    };
    app_edit(&mut engine, ApplicationEdit::Remove { application });
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        matches!(app_edit(&mut engine, ApplicationEdit::Inspect { application }),
        MediaEditResult::ApplicationMissing { application: missing } if missing == application)
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), document);
    for edit in [
        ApplicationEdit::Rename {
            application,
            label: "Missing".into(),
        },
        ApplicationEdit::Remove { application },
        ApplicationEdit::SetEnabled {
            application,
            enabled: false,
        },
        ApplicationEdit::Configure {
            application,
            definition: original.definition.clone(),
        },
    ] {
        assert!(!invoke(&mut engine, MediaEdit::Applications { edit }).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), document);
    }
    assert!(engine.undo().unwrap());
    let MediaEditResult::Application { snapshot: restored } =
        app_edit(&mut engine, ApplicationEdit::Inspect { application })
    else {
        panic!()
    };
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.definition, original.definition);
    assert!(engine.redo().unwrap());
    assert!(matches!(
        app_edit(&mut engine, ApplicationEdit::Inspect { application }),
        MediaEditResult::ApplicationMissing { .. }
    ));

    let manager = crate::media::authoring::manager(&engine).unwrap();
    let wrong_type = ApplicationId(engine.nodes.get(manager).unwrap().node_data().meta.uuid);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Applications {
                edit: ApplicationEdit::Inspect {
                    application: wrong_type
                }
            }
        )
        .success
    );
}
