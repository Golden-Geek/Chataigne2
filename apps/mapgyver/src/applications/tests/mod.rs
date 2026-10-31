use super::{demand::*, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    matrix::protocol::*,
    media::protocol::*,
};
use golden_core::{
    app::{create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    ui_sync::UiEditIntent,
};

mod demand;
mod logical;
mod references;
fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    engine
}
fn invoke(engine: &mut AppEngine, edit: MediaEdit) -> golden_core::ui_sync::UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    )
}
fn edit(engine: &mut AppEngine, operation: MediaEdit) -> MediaEditResult {
    let reply = invoke(engine, operation);
    assert!(reply.success, "{reply:?}");
    serde_json::from_value(reply.result.unwrap().value).unwrap()
}
fn app_edit(engine: &mut AppEngine, operation: ApplicationEdit) -> MediaEditResult {
    edit(engine, MediaEdit::Applications { edit: operation })
}
fn create(engine: &mut AppEngine) -> ApplicationId {
    let MediaEditResult::ApplicationCreated { application } = app_edit(engine, ApplicationEdit::Create { label: None })
    else {
        panic!()
    };
    application
}
fn matrix(engine: &mut AppEngine) -> (MediaId, Vec<ColumnId>) {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::AddColumns { count: 5 },
        },
    ) else {
        panic!()
    };
    (reference.media, columns)
}
fn configure_column(engine: &mut AppEngine, media: MediaId, column: ColumnId, application: ApplicationId) {
    edit(
        engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ConfigureColumn {
                column,
                definition: ColumnDefinition {
                    applications: vec![ColumnApplication {
                        application,
                        pre: 1,
                        post: 1,
                    }],
                    ..Default::default()
                },
            },
        },
    );
}
#[test]
fn shared_definitions_column_copy_missing_refs_history_and_reload_keep_stable_identity() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::DuplicateColumn { column: columns[2] },
        },
    );
    let state = crate::matrix::inspect(&engine, media).unwrap();
    assert_eq!(
        state
            .columns
            .iter()
            .filter(|column| !column.definition.applications.is_empty())
            .count(),
        2
    );
    assert!(
        state
            .columns
            .iter()
            .flat_map(|column| &column.definition.applications)
            .all(|entry| entry.application == application)
    );
    let definition = ApplicationDefinition {
        executable: Some(std::env::current_exe().unwrap().to_string_lossy().into_owned()),
        arguments: vec!["literal & value".into()],
        ..Default::default()
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    let snapshot = super::inspect(&engine, application).unwrap();
    assert_eq!(snapshot.definition, definition);
    let guard = match engine
        .nodes
        .get(engine.node_id_by_uuid(application.0).unwrap())
        .unwrap()
    {
        AppNode::Application(node) => node.guard(),
        _ => panic!(),
    };
    app_edit(
        &mut engine,
        ApplicationEdit::Rename {
            application,
            label: "Shared renderer".into(),
        },
    );
    assert!(
        guard.is_current(),
        "renaming a label must not restart a process configuration"
    );
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    assert!(
        guard.is_current(),
        "resubmitting an unchanged definition preserves its launch guard"
    );
    let text = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&text).unwrap();
    assert_eq!(super::inspect(&restored, application).unwrap().definition, definition);
    assert_eq!(
        crate::matrix::inspect(&restored, media).unwrap().columns,
        crate::matrix::inspect(&engine, media).unwrap().columns
    );
    app_edit(&mut restored, ApplicationEdit::Remove { application });
    assert!(super::inspect(&restored, application).is_err());
    // A non-reference edit retains the missing authored identity; it does not synthesize a new app.
    edit(
        &mut restored,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ConfigureColumn {
                column: columns[2],
                definition: ColumnDefinition {
                    applications: vec![ColumnApplication {
                        application,
                        pre: 1,
                        post: 1,
                    }],
                    transition: Some(TransitionDefinition {
                        duration_ms: 1,
                        curve: TransitionCurve::Linear,
                    }),
                },
            },
        },
    );
    assert!(restored.undo().unwrap());
    assert!(restored.undo().unwrap());
    assert_eq!(super::inspect(&restored, application).unwrap().id, application);
}

#[test]
fn invalid_definitions_associations_and_catalog_queries_are_atomic_and_reads_never_launch() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    let before = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    for definition in [
        ApplicationDefinition {
            executable: Some("relative.exe".into()),
            ..Default::default()
        },
        ApplicationDefinition {
            arguments: vec!["bad\0arg".into()],
            ..Default::default()
        },
        ApplicationDefinition {
            readiness: ApplicationReadiness::Spout {
                sender: "".into(),
                first_frame: true,
                timeout_ms: 1000,
            },
            ..Default::default()
        },
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Applications {
                    edit: ApplicationEdit::Configure {
                        application,
                        definition
                    }
                }
            )
            .success
        );
    }
    let duplicate = vec![
        ColumnApplication {
            application,
            pre: 1,
            post: 1
        };
        2
    ];
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::ConfigureColumn {
                    column: columns[0],
                    definition: ColumnDefinition {
                        applications: duplicate,
                        ..Default::default()
                    }
                }
            }
        )
        .success
    );
    for count in [0, 129] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Applications {
                    edit: ApplicationEdit::InspectCatalog {
                        search: "".into(),
                        start: 0,
                        count
                    }
                }
            )
            .success
        );
    }
    let MediaEditResult::Applications { page } = app_edit(
        &mut engine,
        ApplicationEdit::InspectCatalog {
            search: "APPLICATION".into(),
            start: 0,
            count: 64,
        },
    ) else {
        panic!()
    };
    assert_eq!(page.total, 1);
    assert!(!page.applications[0].configured);
    assert_eq!(engine.undo_len(), before);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    assert!(crate::matrix::inspect(&engine, media).unwrap().cells.is_empty());
}
